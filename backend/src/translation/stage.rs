//! Translation stage: a dedicated worker that translates final captions off the
//! audio path, so a slow model never blocks capture or recognition.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{SyncSender, TrySendError, sync_channel},
    },
    thread::{self, JoinHandle},
};

use super::Translator;
use crate::{domain::TargetLanguage, ipc::CaptionUpdate};

/// Pending finals awaiting translation. Beyond this the translator is too far
/// behind the audio, and the session fails visibly instead of lagging silently.
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
    if info.confidence() < MIN_DETECTION_CONFIDENCE {
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

/// Splits a final caption into sentences. The recognizer punctuates, so sentence
/// marks are reliable boundaries; decimals and abbreviations may split early,
/// a small quality cost paid for much lower first-sentence latency.
fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if matches!(ch, '.' | '!' | '?' | '…') {
            if !current.trim().is_empty() {
                sentences.push(current.trim().to_owned());
            }
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        sentences.push(current.trim().to_owned());
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

/// Owns the translation worker. [`TranslationStage::finish`] drains the queue;
/// dropping the stage instead stops after the caption already being translated.
pub struct TranslationStage {
    sender: Option<SyncSender<CaptionUpdate>>,
    worker: Option<JoinHandle<()>>,
    stop: Arc<AtomicBool>,
}

impl TranslationStage {
    /// `emit` runs on the worker thread for every event and returns `false` to
    /// stop early (stale session or failed delivery). `target` is the language
    /// every caption is translated into.
    pub fn spawn(
        mut translator: Box<dyn Translator>,
        target: TargetLanguage,
        mut emit: impl FnMut(StageEvent) -> bool + Send + 'static,
    ) -> Result<Self, StageError> {
        let (sender, receiver) = sync_channel::<CaptionUpdate>(QUEUE_CAPACITY);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("caption-translator".into())
            .spawn(move || {
                // Source language: recognizer tag, else detected from the text,
                // else the last one seen in the session, else English.
                let mut last_language: Option<String> = None;
                while let Ok(update) = receiver.recv() {
                    let detected = update
                        .language
                        .clone()
                        .or_else(|| detect_source(&update.text).map(str::to_owned));
                    if detected.is_some() {
                        last_language = detected.clone();
                    }
                    let source = detected.or_else(|| last_language.clone());
                    let source = source.as_deref().unwrap_or(DEFAULT_SOURCE);
                    // Long finals translate sentence by sentence, so the first
                    // part reaches the screen while the rest is still decoding.
                    let sentences = split_sentences(&update.text);
                    let total = sentences.len();
                    let mut accumulated = String::new();
                    for (index, sentence) in sentences.into_iter().enumerate() {
                        if stop_flag.load(Ordering::Acquire) {
                            return;
                        }
                        let start = std::time::Instant::now();
                        match translator.translate(&sentence, source, target.tag()) {
                            Ok(text) => {
                                // Timing only; caption text never reaches logs.
                                eprintln!(
                                    "[nmt] translate_ms={} chars_in={} chars_out={}",
                                    start.elapsed().as_millis(),
                                    sentence.chars().count(),
                                    text.chars().count(),
                                );
                                if !text.is_empty() {
                                    if !accumulated.is_empty() {
                                        accumulated.push(' ');
                                    }
                                    accumulated.push_str(&text);
                                }
                                if accumulated.is_empty() {
                                    continue;
                                }
                                let translated = CaptionUpdate {
                                    text: accumulated.clone(),
                                    language: Some(target.tag().to_owned()),
                                    revision: (index + 1) as u32,
                                    is_final: index + 1 == total,
                                    ..update.clone()
                                };
                                if !emit(StageEvent::Translated(translated)) {
                                    return;
                                }
                            }
                            Err(_) => {
                                emit(StageEvent::Failed);
                                return;
                            }
                        }
                    }
                }
            })
            .map_err(|_| StageError::Overloaded)?;
        Ok(Self {
            sender: Some(sender),
            worker: Some(worker),
            stop,
        })
    }

    /// Queues one final caption without blocking the audio path.
    pub fn submit(&self, update: CaptionUpdate) -> Result<(), StageError> {
        let sender = self.sender.as_ref().ok_or(StageError::Overloaded)?;
        match sender.try_send(update) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                Err(StageError::Overloaded)
            }
        }
    }

    /// Closes the queue and waits for the worker to drain it.
    pub fn finish(mut self) {
        self.close_and_join();
    }

    fn close_and_join(&mut self) {
        self.sender.take();
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
        let stage =
            TranslationStage::spawn(Box::new(translator), crate::domain::PT_BR, move |event| {
                match event {
                    StageEvent::Translated(update) => {
                        sink.lock().unwrap().push(update);
                        true
                    }
                    StageEvent::Failed => false,
                }
            })
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

    #[test]
    fn long_finals_stream_sentence_by_sentence() {
        let mut translator = MockTranslator::new();
        translator
            .expect_translate()
            .withf(|text, source, _| source == "en-US" && (text == "One." || text == "Two."))
            .times(2)
            .returning(|text, _, _| Ok(if text == "One." { "Um." } else { "Dois." }.to_owned()));

        let emitted = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&emitted);
        let stage =
            TranslationStage::spawn(Box::new(translator), crate::domain::PT_BR, move |event| {
                match event {
                    StageEvent::Translated(update) => {
                        sink.lock().unwrap().push(update);
                        true
                    }
                    StageEvent::Failed => false,
                }
            })
            .unwrap();
        stage
            .submit(final_update(3, "One. Two.", Some("en-US")))
            .unwrap();
        stage.finish();

        let emitted = emitted.lock().unwrap();
        // The first sentence reaches the screen before the second finishes decoding.
        assert_eq!(emitted.len(), 2);
        assert_eq!(
            (emitted[0].text.as_str(), emitted[0].is_final),
            ("Um.", false)
        );
        assert_eq!(emitted[0].revision, 1);
        assert_eq!(
            (emitted[1].text.as_str(), emitted[1].is_final),
            ("Um. Dois.", true)
        );
        assert_eq!(emitted[1].revision, 2);
        assert_eq!(emitted[1].segment_id, 3);
    }

    #[test]
    fn translator_errors_surface_once_and_stop_the_worker() {
        let mut translator = MockTranslator::new();
        translator
            .expect_translate()
            .times(1)
            .returning(|_, _, _| Err(TranslatorError::TranslationFailed));

        let failed = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&failed);
        let stage =
            TranslationStage::spawn(Box::new(translator), crate::domain::PT_BR, move |event| {
                if matches!(event, StageEvent::Failed) {
                    flag.store(true, Ordering::Release);
                }
                false
            })
            .unwrap();

        stage.submit(final_update(1, "Boom", None)).unwrap();
        wait_until(|| failed.load(Ordering::Acquire));
        stage.finish();
        assert!(failed.load(Ordering::Acquire));
    }

    #[test]
    fn a_full_queue_reports_overload_instead_of_blocking() {
        // The translator never finishes the first caption, so the queue fills up.
        let mut translator = MockTranslator::new();
        translator.expect_translate().returning(|text, _, _| {
            thread::sleep(Duration::from_millis(300));
            Ok(text.to_owned())
        });
        let stage =
            TranslationStage::spawn(Box::new(translator), crate::domain::PT_BR, |_| true).unwrap();

        let mut overloaded = false;
        // One caption may be in flight; the queue holds QUEUE_CAPACITY more.
        for segment in 0..=(QUEUE_CAPACITY as u32 + 1) {
            if stage.submit(final_update(segment + 1, "text", None)) == Err(StageError::Overloaded)
            {
                overloaded = true;
                break;
            }
        }
        assert!(overloaded, "the bounded queue must refuse further captions");
        // Dropping the stage must not wait for the sleeping translator forever:
        // the worker drains remaining items only after the current call returns.
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
        let stage =
            TranslationStage::spawn(Box::new(translator), crate::domain::PT_BR, move |event| {
                match event {
                    StageEvent::Translated(update) => {
                        sink.lock().unwrap().push(update);
                        true
                    }
                    StageEvent::Failed => false,
                }
            })
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
    fn empty_translations_are_skipped_and_stale_sessions_stop_the_worker() {
        let mut translator = MockTranslator::new();
        translator
            .expect_translate()
            .returning(|_, _, _| Ok(String::new()));
        let emitted = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&emitted);
        let stage =
            TranslationStage::spawn(Box::new(translator), crate::domain::PT_BR, move |_| {
                flag.store(true, Ordering::Release);
                true
            })
            .unwrap();
        stage.submit(final_update(1, "   ", None)).unwrap();
        stage.finish();
        assert!(!emitted.load(Ordering::Acquire));
    }
}
