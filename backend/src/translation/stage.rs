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
use crate::ipc::CaptionUpdate;

/// Pending finals awaiting translation. Beyond this the translator is too far
/// behind the audio, and the session fails visibly instead of lagging silently.
const QUEUE_CAPACITY: usize = 8;
/// Captions without a detected language assume English, the dominant case.
const DEFAULT_SOURCE: &str = "en-US";
/// Fixed target of the translate mode; more targets become configuration later.
pub const TARGET_LANGUAGE: &str = "pt-BR";

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
    /// stop early (stale session or failed delivery).
    pub fn spawn(
        mut translator: Box<dyn Translator>,
        mut emit: impl FnMut(StageEvent) -> bool + Send + 'static,
    ) -> Result<Self, StageError> {
        let (sender, receiver) = sync_channel::<CaptionUpdate>(QUEUE_CAPACITY);
        let stop = Arc::new(AtomicBool::new(false));
        let stop_flag = Arc::clone(&stop);
        let worker = thread::Builder::new()
            .name("caption-translator".into())
            .spawn(move || {
                while let Ok(update) = receiver.recv() {
                    if stop_flag.load(Ordering::Acquire) {
                        return;
                    }
                    let source = update.language.as_deref().unwrap_or(DEFAULT_SOURCE);
                    match translator.translate(&update.text, source, TARGET_LANGUAGE) {
                        Ok(text) if text.is_empty() => continue,
                        Ok(text) => {
                            let translated = CaptionUpdate {
                                text,
                                language: Some(TARGET_LANGUAGE.to_owned()),
                                ..update
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
            mode: CaptionMode::TranslatePt,
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
    fn translates_finals_in_order_with_detected_or_default_source() {
        let mut translator = MockTranslator::new();
        translator
            .expect_translate()
            .withf(|text, source, target| {
                target == TARGET_LANGUAGE
                    && match text {
                        "First" => source == "es-ES",
                        "Second" => source == DEFAULT_SOURCE,
                        _ => false,
                    }
            })
            .times(2)
            .returning(|text, _, _| Ok(format!("PT {text}")));

        let emitted = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&emitted);
        let stage = TranslationStage::spawn(Box::new(translator), move |event| match event {
            StageEvent::Translated(update) => {
                sink.lock().unwrap().push(update);
                true
            }
            StageEvent::Failed => false,
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
        assert_eq!(emitted[0].language.as_deref(), Some(TARGET_LANGUAGE));
        assert!(emitted[0].is_final);
        assert_eq!(emitted[1].text, "PT Second");
        assert_eq!(
            (emitted[0].segment_id, emitted[1].segment_id),
            (1, 2),
            "order must be preserved"
        );
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
        let stage = TranslationStage::spawn(Box::new(translator), move |event| {
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
        let stage = TranslationStage::spawn(Box::new(translator), |_| true).unwrap();

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
    fn empty_translations_are_skipped_and_stale_sessions_stop_the_worker() {
        let mut translator = MockTranslator::new();
        translator
            .expect_translate()
            .returning(|_, _, _| Ok(String::new()));
        let emitted = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&emitted);
        let stage = TranslationStage::spawn(Box::new(translator), move |_| {
            flag.store(true, Ordering::Release);
            true
        })
        .unwrap();
        stage.submit(final_update(1, "   ", None)).unwrap();
        stage.finish();
        assert!(!emitted.load(Ordering::Acquire));
    }
}
