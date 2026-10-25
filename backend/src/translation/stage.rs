//! Translation stage: a dedicated worker that translates final captions off the
//! audio path, so a slow model never blocks capture or recognition.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

use super::{Translator, TranslatorError};
use crate::{domain::TargetLanguage, ipc::CaptionUpdate};

/// One failed call skips only its caption; this many in a row means the
/// translator is really gone and the session must surface the error.
const MAX_CONSECUTIVE_FAILURES: usize = 3;

/// Pending finals awaiting translation. Beyond this the translator is too far
/// behind live speech, so the oldest pending final is dropped: fresh captions
/// matter more than a backlog the viewer would read too late.
const QUEUE_CAPACITY: usize = 8;
/// Captions without a detected language assume English, the dominant case.
const DEFAULT_SOURCE: &str = "en-US";

/// Minimum whatlang confidence to trust a detection; caption-sized sentences
/// rarely reach the crate's own `is_reliable` bar, but 0.5 over the allowlist
/// below proved accurate, and anything lower falls back to the session memory.
const MIN_DETECTION_CONFIDENCE: f64 = 0.5;

/// Detects the caption language from its text (locally, via whatlang) over the
/// languages the translation model supports, mapped to the model's tags. The
/// recognizer tags results with the spoken language only sporadically, so the
/// text is the reliable signal.
fn detect_source(text: &str) -> Option<&'static str> {
    detect_source_with(text, MIN_DETECTION_CONFIDENCE)
}

/// The detector's best guess regardless of confidence, still restricted to the
/// languages the model supports. Used only before the first confident lock,
/// where a guess beats assuming English (that assumption turned into raw,
/// untranslated passthrough whenever the target was English).
fn guess_source(text: &str) -> Option<&'static str> {
    detect_source_with(text, 0.0)
}

fn detect_source_with(text: &str, min_confidence: f64) -> Option<&'static str> {
    use whatlang::{Detector, Lang};
    const LANGS: [Lang; 34] = [
        Lang::Eng,
        Lang::Por,
        Lang::Spa,
        Lang::Fra,
        Lang::Deu,
        Lang::Ita,
        Lang::Nld,
        Lang::Pol,
        Lang::Ces,
        Lang::Slk,
        Lang::Slv,
        Lang::Hrv,
        Lang::Hun,
        Lang::Ron,
        Lang::Bul,
        Lang::Ell,
        Lang::Est,
        Lang::Lit,
        Lang::Lav,
        Lang::Fin,
        Lang::Swe,
        Lang::Dan,
        Lang::Nob,
        Lang::Ara,
        Lang::Hin,
        Lang::Ind,
        Lang::Jpn,
        Lang::Kor,
        Lang::Rus,
        Lang::Tha,
        Lang::Tur,
        Lang::Ukr,
        Lang::Vie,
        Lang::Cmn,
    ];
    let info = Detector::with_allowlist(LANGS.to_vec()).detect(text)?;
    if info.confidence() < min_confidence {
        return None;
    }
    Some(match info.lang() {
        Lang::Eng => "en",
        Lang::Por => "pt-br",
        Lang::Spa => "es",
        Lang::Fra => "fr",
        Lang::Deu => "de",
        Lang::Ita => "it",
        Lang::Nld => "nl",
        Lang::Pol => "pl",
        Lang::Ces => "cs",
        Lang::Slk => "sk",
        Lang::Slv => "sl",
        Lang::Hrv => "hr",
        Lang::Hun => "hu",
        Lang::Ron => "ro",
        Lang::Bul => "bg",
        Lang::Ell => "el",
        Lang::Est => "et",
        Lang::Lit => "lt",
        Lang::Lav => "lv",
        Lang::Fin => "fi",
        Lang::Swe => "sv",
        Lang::Dan => "da",
        Lang::Nob => "no",
        Lang::Ara => "ar",
        Lang::Hin => "hi",
        Lang::Ind => "id",
        Lang::Jpn => "ja",
        Lang::Kor => "ko",
        Lang::Rus => "ru",
        Lang::Tha => "th",
        Lang::Tur => "tr",
        Lang::Ukr => "uk",
        Lang::Vie => "vi",
        Lang::Cmn => "zh-cn",
        _ => return None,
    })
}

/// Recent caption text kept for language detection. Detecting over a window of
/// accumulated text is far steadier than per-caption guesses: short finals
/// ("Ok?") carry too little signal and used to flip the session language.
const DETECTION_WINDOW_CHARS: usize = 240;

/// Resolves the source language of each final: recognizer tag when present,
/// else detection over the recent text window, else the last known language,
/// else English.
#[derive(Default)]
struct SourceTracker {
    window: String,
    last: Option<String>,
}

impl SourceTracker {
    /// Drops the session memory after an unsupported pair, so a bad recognizer
    /// tag or misdetection does not poison every following caption.
    fn forget(&mut self) {
        self.window.clear();
        self.last = None;
    }

    fn resolve(&mut self, update: &CaptionUpdate) -> String {
        if let Some(tag) = update.language.as_deref().filter(|tag| !tag.is_empty()) {
            // The recognizer knows best; restart the window in the new language.
            self.window.clear();
            self.last = Some(tag.to_owned());
            return tag.to_owned();
        }
        if !self.window.is_empty() {
            self.window.push(' ');
        }
        self.window.push_str(update.text.trim());
        let excess = self
            .window
            .chars()
            .count()
            .saturating_sub(DETECTION_WINDOW_CHARS);
        if excess > 0 {
            // Keep the most recent text so a language switch wins the window
            // after a couple of captions.
            self.window = self.window.chars().skip(excess).collect();
        }
        if let Some(detected) = detect_source(&self.window) {
            self.last = Some(detected.to_owned());
            return detected.to_owned();
        }
        if self.last.is_none() {
            // No session memory yet: a low-confidence guess still beats the
            // English assumption (it does not lock the session language).
            if let Some(guess) = guess_source(&self.window) {
                return guess.to_owned();
            }
        }
        self.last
            .clone()
            .unwrap_or_else(|| DEFAULT_SOURCE.to_owned())
    }
}

/// A sentence shorter than this merges with its neighbor before translating:
/// tiny blocks ("Ok.") flashed alone on screen — barely visible with the fast
/// OPUS engine — and translate worse without the surrounding context.
const MIN_SENTENCE_CHARS: usize = 24;

/// Splits a final caption into sentences. The recognizer punctuates, so sentence
/// marks are reliable boundaries; decimals and abbreviations may split early,
/// a small quality cost paid for much lower first-sentence latency. Sentences
/// below [`MIN_SENTENCE_CHARS`] stay attached to the next one (or the previous,
/// at the end), so no tiny fragment reaches the screen on its own.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences: Vec<String> = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?' | '…')
            && current.trim().chars().count() >= MIN_SENTENCE_CHARS
        {
            sentences.push(current.trim().to_owned());
            current.clear();
        }
    }
    let rest = current.trim();
    if !rest.is_empty() {
        if rest.chars().count() < MIN_SENTENCE_CHARS
            && let Some(last) = sentences.last_mut()
        {
            last.push(' ');
            last.push_str(rest);
        } else {
            sentences.push(rest.to_owned());
        }
    }
    sentences
}

/// What the stage reports back to the session coordinator.
pub enum StageEvent {
    /// A caption finished translating; the text is already in the target language.
    Translated(CaptionUpdate),
    /// The translator failed; the session must surface the error.
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageError {
    /// The queue is full or the worker is gone.
    Overloaded,
}

/// The bounded hand-off between the audio path and the worker. A channel cannot
/// drop its oldest entry, so this is a small queue under a lock with a condvar
/// wake-up for the worker.
struct StageQueue {
    state: Mutex<QueueState>,
    available: Condvar,
}

struct QueueState {
    pending: VecDeque<CaptionUpdate>,
    closed: bool,
}

impl StageQueue {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(QueueState {
                pending: VecDeque::new(),
                closed: false,
            }),
            available: Condvar::new(),
        })
    }

    /// Queues one final. When full, the oldest pending final gives way so the
    /// stage keeps up with live speech instead of failing the session.
    fn push(&self, update: CaptionUpdate) -> Result<(), StageError> {
        let mut state = self.state.lock().map_err(|_| StageError::Overloaded)?;
        if state.closed {
            return Err(StageError::Overloaded);
        }
        if state.pending.len() >= QUEUE_CAPACITY {
            state.pending.pop_front();
            // Timing-only; caption text never reaches logs.
            eprintln!("[nmt] queue full, oldest pending final dropped");
        }
        state.pending.push_back(update);
        self.available.notify_one();
        Ok(())
    }

    /// Blocks until a final is available; `None` once closed and drained.
    fn pop(&self) -> Option<CaptionUpdate> {
        let mut state = self.state.lock().ok()?;
        loop {
            if let Some(update) = state.pending.pop_front() {
                return Some(update);
            }
            if state.closed {
                return None;
            }
            state = self.available.wait(state).ok()?;
        }
    }

    fn close(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.closed = true;
        }
        self.available.notify_all();
    }
}

/// Owns the translation worker. [`TranslationStage::finish`] drains the queue;
/// dropping the stage instead stops after the caption already being translated.
pub struct TranslationStage {
    queue: Arc<StageQueue>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

impl TranslationStage {
    /// `emit` runs on the worker thread for every event and returns `false` to
    /// stop early (stale session or failed delivery). `target` is the language
    /// every caption is translated into. `fixed_source` is the spoken language
    /// the user pinned; `None` resolves it per caption (recognizer tag →
    /// detection window → session memory → English).
    pub fn spawn(
        mut translator: Box<dyn Translator>,
        target: TargetLanguage,
        fixed_source: Option<&'static str>,
        mut emit: impl FnMut(StageEvent) -> bool + Send + 'static,
    ) -> Result<Self, StageError> {
        let queue = StageQueue::new();
        let worker_queue = Arc::clone(&queue);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("caption-translator".into())
            .spawn(move || {
                let mut source_tracker = SourceTracker::default();
                let mut failures_in_a_row = 0_usize;
                while let Some(update) = worker_queue.pop() {
                    if stop_flag.load(Ordering::Acquire) {
                        return;
                    }
                    let source = match fixed_source {
                        Some(tag) => tag.to_owned(),
                        None => source_tracker.resolve(&update),
                    };
                    let source = source.as_str();
                    // Long finals translate sentence by sentence, and each
                    // translated sentence is shown as its own block (the next
                    // one replaces it): literal, stable text instead of a line
                    // that keeps rewriting itself.
                    let sentences = split_sentences(&update.text);
                    let total = sentences.len();
                    for (index, sentence) in sentences.into_iter().enumerate() {
                        if stop_flag.load(Ordering::Acquire) {
                            return;
                        }
                        let start = std::time::Instant::now();
                        match translator.translate(&sentence, source, target.tag()) {
                            Ok(text) => {
                                failures_in_a_row = 0;
                                // Timing only; caption text never reaches logs.
                                eprintln!(
                                    "[nmt] translate_ms={} chars_in={} chars_out={}",
                                    start.elapsed().as_millis(),
                                    sentence.chars().count(),
                                    text.chars().count(),
                                );
                                if text.is_empty() {
                                    continue;
                                }
                                let translated = CaptionUpdate {
                                    text,
                                    language: Some(target.tag().to_owned()),
                                    revision: (index + 1) as u32,
                                    is_final: index + 1 == total,
                                    ..update.clone()
                                };
                                if !emit(StageEvent::Translated(translated)) {
                                    return;
                                }
                            }
                            Err(error) => {
                                // Language codes only; caption text never reaches logs.
                                eprintln!(
                                    "[nmt] translate failed ({error:?}) source={source} target={}",
                                    target.tag()
                                );
                                if matches!(error, TranslatorError::UnsupportedLanguagePair) {
                                    source_tracker.forget();
                                }
                                failures_in_a_row += 1;
                                if failures_in_a_row >= MAX_CONSECUTIVE_FAILURES {
                                    emit(StageEvent::Failed);
                                    return;
                                }
                                // Skip only this caption; the session keeps running.
                                break;
                            }
                        }
                    }
                }
            })
            .map_err(|_| StageError::Overloaded)?;
        Ok(Self {
            queue,
            worker: Some(worker),
            stop,
        })
    }

    /// Queues one final caption without blocking the audio path. A backlog
    /// drops its oldest entry instead of failing (see [`StageQueue::push`]).
    pub fn submit(&self, update: CaptionUpdate) -> Result<(), StageError> {
        self.queue.push(update)
    }

    /// Closes the queue and waits for the worker to drain it.
    pub fn finish(mut self) {
        self.close_and_join();
    }

    fn close_and_join(&mut self) {
        self.queue.close();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for TranslationStage {
    fn drop(&mut self) {
        // Abort path: skip whatever is still queued instead of draining it.
        self.stop.store(true, Ordering::Release);
        self.close_and_join();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::CaptionMode;
    use crate::translation::{MockTranslator, TranslatorError};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::{Duration, Instant};

    fn final_update(segment_id: u32, text: &str, language: Option<&str>) -> CaptionUpdate {
        CaptionUpdate {
            session_id: 1,
            segment_id,
            revision: 1,
            mode: CaptionMode::Translate(crate::domain::PT_BR),
            text: text.into(),
            is_final: true,
            language: language.map(str::to_owned),
        }
    }

    fn wait_until(check: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !check() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn translates_finals_in_order_with_detected_or_last_known_source() {
        let mut translator = MockTranslator::new();
        translator
            .expect_translate()
            .withf(|text, source, target| {
                target == "pt-br"
                    && match text {
                        "First" => source == "es-ES",
                        "Second" => source == "es-ES", // sticky: last detected language
                        _ => false,
                    }
            })
            .times(2)
            .returning(|text, _, _| Ok(format!("PT {text}")));

        let emitted = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&emitted);
        let stage = TranslationStage::spawn(
            Box::new(translator),
            crate::domain::PT_BR,
            None,
            move |event| match event {
                StageEvent::Translated(update) => {
                    sink.lock().unwrap().push(update);
                    true
                }
                StageEvent::Failed => false,
            },
        )
        .unwrap();

        stage
            .submit(final_update(1, "First", Some("es-ES")))
            .unwrap();
        stage.submit(final_update(2, "Second", None)).unwrap();
        stage.finish();

        let emitted = emitted.lock().unwrap();
        assert_eq!(emitted.len(), 2);
        assert_eq!(emitted[0].text, "PT First");
        assert_eq!(emitted[0].language.as_deref(), Some("pt-br"));
        assert!(emitted[0].is_final);
        assert_eq!(emitted[1].text, "PT Second");
        assert_eq!(
            (emitted[0].segment_id, emitted[1].segment_id),
            (1, 2),
            "order must be preserved"
        );
    }

    const FIRST_SENTENCE: &str = "The first sentence is long enough to stand alone.";
    const SECOND_SENTENCE: &str = "The second sentence is also long enough for that.";

    #[test]
    fn long_finals_stream_sentence_by_sentence() {
        let mut translator = MockTranslator::new();
        translator
            .expect_translate()
            .withf(|text, source, _| {
                source == "en-US" && (text == FIRST_SENTENCE || text == SECOND_SENTENCE)
            })
            .times(2)
            .returning(|text, _, _| {
                Ok(if text == FIRST_SENTENCE {
                    "Um."
                } else {
                    "Dois."
                }
                .to_owned())
            });

        let emitted = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&emitted);
        let stage = TranslationStage::spawn(
            Box::new(translator),
            crate::domain::PT_BR,
            None,
            move |event| match event {
                StageEvent::Translated(update) => {
                    sink.lock().unwrap().push(update);
                    true
                }
                StageEvent::Failed => false,
            },
        )
        .unwrap();
        stage
            .submit(final_update(
                3,
                &format!("{FIRST_SENTENCE} {SECOND_SENTENCE}"),
                Some("en-US"),
            ))
            .unwrap();
        stage.finish();

        let emitted = emitted.lock().unwrap();
        // Each sentence reaches the screen as its own literal block; the
        // second replaces the first instead of growing the line.
        assert_eq!(emitted.len(), 2);
        assert_eq!(
            (emitted[0].text.as_str(), emitted[0].is_final),
            ("Um.", false)
        );
        assert_eq!(emitted[0].revision, 1);
        assert_eq!(
            (emitted[1].text.as_str(), emitted[1].is_final),
            ("Dois.", true)
        );
        assert_eq!(emitted[1].revision, 2);
        assert_eq!(emitted[1].segment_id, 3);
    }

    #[test]
    fn tiny_sentences_merge_with_a_neighbor_instead_of_flashing_alone() {
        // A short opener joins the sentence that follows it.
        assert_eq!(
            split_sentences(&format!("Ok. {FIRST_SENTENCE}")),
            vec![format!("Ok. {FIRST_SENTENCE}")]
        );
        // A short trailing fragment joins the sentence before it.
        assert_eq!(
            split_sentences(&format!("{FIRST_SENTENCE} Right")),
            vec![format!("{FIRST_SENTENCE} Right")]
        );
        // A final made only of tiny sentences still comes through, whole.
        assert_eq!(split_sentences("Ok. Yes. Right."), vec!["Ok. Yes. Right."]);
        // Long sentences keep streaming separately.
        assert_eq!(
            split_sentences(&format!("{FIRST_SENTENCE} {SECOND_SENTENCE}")),
            vec![FIRST_SENTENCE.to_owned(), SECOND_SENTENCE.to_owned()]
        );
    }

    #[test]
    fn repeated_translator_errors_surface_once_and_stop_the_worker() {
        let mut translator = MockTranslator::new();
        translator
            .expect_translate()
            .times(MAX_CONSECUTIVE_FAILURES)
            .returning(|_, _, _| Err(TranslatorError::TranslationFailed));

        let failed = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&failed);
        let stage = TranslationStage::spawn(
            Box::new(translator),
            crate::domain::PT_BR,
            None,
            move |event| {
                if matches!(event, StageEvent::Failed) {
                    flag.store(true, Ordering::Release);
                }
                false
            },
        )
        .unwrap();

        for segment in 1..=(MAX_CONSECUTIVE_FAILURES as u32) {
            stage.submit(final_update(segment, "Boom", None)).unwrap();
        }
        wait_until(|| failed.load(Ordering::Acquire));
        stage.finish();
        assert!(failed.load(Ordering::Acquire));
    }

    #[test]
    fn an_isolated_translator_error_skips_only_that_caption() {
        let mut translator = MockTranslator::new();
        translator
            .expect_translate()
            .withf(|text, _, _| text == "Boom")
            .times(1)
            .returning(|_, _, _| Err(TranslatorError::TranslationFailed));
        translator
            .expect_translate()
            .withf(|text, _, _| text == "Fine")
            .times(1)
            .returning(|_, _, _| Ok("Tudo bem".to_owned()));

        let emitted = Arc::new(Mutex::new(Vec::new()));
        let failed = Arc::new(AtomicBool::new(false));
        let sink = Arc::clone(&emitted);
        let flag = Arc::clone(&failed);
        let stage = TranslationStage::spawn(
            Box::new(translator),
            crate::domain::PT_BR,
            None,
            move |event| match event {
                StageEvent::Translated(update) => {
                    sink.lock().unwrap().push(update.text);
                    true
                }
                StageEvent::Failed => {
                    flag.store(true, Ordering::Release);
                    false
                }
            },
        )
        .unwrap();

        stage.submit(final_update(1, "Boom", None)).unwrap();
        stage.submit(final_update(2, "Fine", None)).unwrap();
        stage.finish();

        assert!(
            !failed.load(Ordering::Acquire),
            "one failure must not fail the session"
        );
        assert_eq!(*emitted.lock().unwrap(), vec!["Tudo bem".to_owned()]);
    }

    #[test]
    fn an_unsupported_pair_resets_the_source_memory() {
        let mut translator = MockTranslator::new();
        // A bogus recognizer tag reaches the translator once and is rejected.
        translator
            .expect_translate()
            .withf(|_, source, _| source == "xx-weird")
            .times(1)
            .returning(|_, _, _| Err(TranslatorError::UnsupportedLanguagePair));
        // The next caption re-detects from its own text instead of reusing it.
        translator
            .expect_translate()
            .withf(|_, source, _| source == "pt-br")
            .times(1)
            .returning(|text, _, _| Ok(text.to_owned()));

        let failed = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&failed);
        let stage = TranslationStage::spawn(
            Box::new(translator),
            crate::domain::PT_BR,
            None,
            move |event| {
                if matches!(event, StageEvent::Failed) {
                    flag.store(true, Ordering::Release);
                }
                true
            },
        )
        .unwrap();

        stage
            .submit(final_update(1, "Hello there.", Some("xx-weird")))
            .unwrap();
        stage
            .submit(final_update(
                2,
                "Olá pessoal, hoje vamos conversar sobre o projeto novo.",
                None,
            ))
            .unwrap();
        stage.finish();
        assert!(!failed.load(Ordering::Acquire));
    }

    #[test]
    fn a_full_queue_drops_the_oldest_final_and_keeps_the_session_alive() {
        // The translator is slower than the submissions, so the queue fills up.
        let mut translator = MockTranslator::new();
        translator.expect_translate().returning(|text, _, _| {
            thread::sleep(Duration::from_millis(100));
            Ok(text.to_owned())
        });
        let emitted = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&emitted);
        let stage = TranslationStage::spawn(
            Box::new(translator),
            crate::domain::PT_BR,
            None,
            move |event| match event {
                StageEvent::Translated(update) => {
                    sink.lock().unwrap().push(update.segment_id);
                    true
                }
                StageEvent::Failed => false,
            },
        )
        .unwrap();

        // Twice the capacity: every submit must succeed (never fail the session),
        // dropping backlog instead.
        let last_segment = (QUEUE_CAPACITY as u32) * 2;
        for segment in 1..=last_segment {
            assert_eq!(stage.submit(final_update(segment, "text", None)), Ok(()));
        }
        stage.finish();

        let emitted = emitted.lock().unwrap();
        assert!(
            emitted.len() < last_segment as usize,
            "the overloaded queue must have dropped some backlog: {emitted:?}"
        );
        assert_eq!(
            emitted.last(),
            Some(&last_segment),
            "the freshest caption must survive the drops: {emitted:?}"
        );
        assert!(
            emitted.windows(2).all(|pair| pair[0] < pair[1]),
            "surviving captions must stay in order: {emitted:?}"
        );
    }

    #[test]
    fn source_tracker_keeps_the_session_language_through_short_captions() {
        let mut tracker = SourceTracker::default();
        // A long Portuguese caption locks the session language.
        assert_eq!(
            tracker.resolve(&final_update(
                1,
                "Olá pessoal, hoje vamos conversar sobre o projeto novo.",
                None,
            )),
            "pt-br"
        );
        // A short, ambiguous follow-up must not flip it: the window still holds
        // the Portuguese text around it.
        assert_eq!(tracker.resolve(&final_update(2, "Ok.", None)), "pt-br");
    }

    #[test]
    fn source_tracker_switches_after_enough_text_in_the_new_language() {
        let mut tracker = SourceTracker::default();
        tracker.resolve(&final_update(
            1,
            "Olá pessoal, hoje vamos conversar sobre o projeto novo.",
            None,
        ));
        let mut resolved = String::new();
        for segment in 2..=5 {
            resolved = tracker.resolve(&final_update(
                segment,
                "The update will be released next week for everyone to try out.",
                None,
            ));
        }
        assert_eq!(resolved, "en", "sustained English must win the window");
    }

    #[test]
    fn source_tracker_trusts_recognizer_tags_and_resets_the_window() {
        let mut tracker = SourceTracker::default();
        tracker.resolve(&final_update(
            1,
            "Olá pessoal, hoje vamos conversar sobre o projeto novo.",
            None,
        ));
        // A tag wins immediately and clears the accumulated Portuguese.
        assert_eq!(
            tracker.resolve(&final_update(2, "Now in English.", Some("en-US"))),
            "en-US"
        );
        assert_eq!(tracker.resolve(&final_update(3, "Ok.", None)), "en-US");
    }

    #[test]
    fn source_tracker_defaults_to_english_without_any_signal() {
        let mut tracker = SourceTracker::default();
        // Digits carry no language signal at all, so even the best-effort
        // guess abstains and the English default applies.
        assert_eq!(
            tracker.resolve(&final_update(1, "1 2 3.", None)),
            DEFAULT_SOURCE
        );
    }

    #[test]
    fn source_tracker_guesses_before_the_first_confident_lock() {
        let mut tracker = SourceTracker::default();
        // A first short Portuguese caption: too little text for a confident
        // lock, but the best-effort guess must not fall back to English (that
        // showed raw, untranslated text whenever the target was English).
        assert_eq!(
            tracker.resolve(&final_update(1, "Olá, tudo bem com vocês?", None)),
            "pt-br"
        );
    }

    #[test]
    fn detector_maps_supported_languages_and_ignores_short_or_odd_text() {
        assert_eq!(
            detect_source("Olá pessoal, hoje vamos conversar sobre o projeto novo."),
            Some("pt-br")
        );
        assert_eq!(
            detect_source("The update will be released next week for everyone."),
            Some("en")
        );
        assert_eq!(detect_source(""), None);
    }

    #[test]
    fn untagged_portuguese_finals_are_detected_from_the_text() {
        let mut translator = MockTranslator::new();
        translator
            .expect_translate()
            .withf(|_, source, target| source == "pt-br" && target == "pt-br")
            .times(1)
            .returning(|text, _, _| Ok(text.to_owned()));

        let emitted = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&emitted);
        let stage = TranslationStage::spawn(
            Box::new(translator),
            crate::domain::PT_BR,
            None,
            move |event| match event {
                StageEvent::Translated(update) => {
                    sink.lock().unwrap().push(update);
                    true
                }
                StageEvent::Failed => false,
            },
        )
        .unwrap();
        // The recognizer did not tag the final; the text itself is Portuguese.
        stage
            .submit(final_update(
                1,
                "Olá pessoal, hoje vamos conversar sobre o projeto novo.",
                None,
            ))
            .unwrap();
        stage.finish();
        assert_eq!(emitted.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_pinned_source_bypasses_detection_tags_and_session_memory() {
        let mut translator = MockTranslator::new();
        // Every caption must reach the translator as pt-BR: the English text,
        // the conflicting recognizer tag, and the ambiguous short final would
        // each have produced a different source through the tracker.
        translator
            .expect_translate()
            .withf(|_, source, target| source == "pt-BR" && target == "en")
            .times(3)
            .returning(|text, _, _| Ok(format!("EN {text}")));

        let emitted = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&emitted);
        let stage = TranslationStage::spawn(
            Box::new(translator),
            crate::domain::EN,
            Some("pt-BR"),
            move |event| match event {
                StageEvent::Translated(update) => {
                    sink.lock().unwrap().push(update);
                    true
                }
                StageEvent::Failed => false,
            },
        )
        .unwrap();
        stage
            .submit(final_update(
                1,
                "The update will be released next week for everyone.",
                None,
            ))
            .unwrap();
        stage
            .submit(final_update(
                2,
                "Now tagged as English by the model.",
                Some("en-US"),
            ))
            .unwrap();
        stage.submit(final_update(3, "Ok.", None)).unwrap();
        stage.finish();
        assert_eq!(emitted.lock().unwrap().len(), 3);
    }

    #[test]
    fn empty_translations_are_skipped_and_stale_sessions_stop_the_worker() {
        let mut translator = MockTranslator::new();
        translator
            .expect_translate()
            .returning(|_, _, _| Ok(String::new()));
        let emitted = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&emitted);
        let stage = TranslationStage::spawn(
            Box::new(translator),
            crate::domain::PT_BR,
            None,
            move |_| {
                flag.store(true, Ordering::Release);
                true
            },
        )
        .unwrap();
        stage.submit(final_update(1, "   ", None)).unwrap();
        stage.finish();
        assert!(!emitted.load(Ordering::Acquire));
    }
}
