use crate::{
    domain::CaptionMode,
    ipc::CaptionUpdate,
    pipeline::{AudioProcessor, Cancellation, Message, PipelineError},
};

/// Longest caption text forwarded to the UI; longer engine output is treated as a failure.
pub const MAX_CAPTION_CHARS: usize = 4_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecognizerError {
    LibraryUnavailable,
    ModelUnavailable,
    InvalidAudio,
    RecognitionFailed,
}

/// One recognition result: the whole utterance so far, or its final text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechResult {
    pub text: String,
    pub is_final: bool,
    /// Language detected by the model (BCP-47); only final results carry it.
    pub language: Option<String>,
}

/// A streaming speech recognizer fed with 16 kHz mono audio. It detects speech and the end of
/// each utterance itself, so no external VAD or segmentation is needed.
#[cfg_attr(test, mockall::automock)]
pub trait StreamingRecognizer: Send {
    fn push(&mut self, samples: &[f32]) -> Result<Vec<SpeechResult>, RecognizerError>;
    /// Flushes buffered audio when capture stops.
    fn finish(&mut self) -> Result<Vec<SpeechResult>, RecognizerError>;
}

/// Turns recognizer results into caption updates: each utterance is a segment, and every new
/// text for it is a higher revision.
pub struct StreamingProcessor<R: StreamingRecognizer> {
    session_id: u32,
    mode: CaptionMode,
    recognizer: R,
    segment_id: u32,
    revision: u32,
    last_text: String,
}

impl<R: StreamingRecognizer> StreamingProcessor<R> {
    pub fn new(session_id: u32, mode: CaptionMode, recognizer: R) -> Self {
        Self {
            session_id,
            mode,
            recognizer,
            segment_id: 1,
            revision: 0,
            last_text: String::new(),
        }
    }

    fn updates(
        &mut self,
        results: Vec<SpeechResult>,
    ) -> Result<Option<Vec<CaptionUpdate>>, PipelineError> {
        let mut updates = Vec::new();
        for result in results {
            let text = result.text.trim();
            if text.len() > MAX_CAPTION_CHARS {
                return Err(PipelineError::ProcessorFailed);
            }
            // Interim results repeat while no new word is decoded; a final always counts.
            let unchanged = !result.is_final && text == self.last_text;
            if !text.is_empty() && !unchanged {
                self.revision = self
                    .revision
                    .checked_add(1)
                    .ok_or(PipelineError::CounterExhausted)?;
                updates.push(CaptionUpdate {
                    session_id: self.session_id,
                    segment_id: self.segment_id,
                    revision: self.revision,
                    mode: self.mode,
                    text: text.to_owned(),
                    is_final: result.is_final,
                    language: result.language.clone(),
                });
                self.last_text = text.to_owned();
            }
            if result.is_final {
                self.segment_id = self
                    .segment_id
                    .checked_add(1)
                    .ok_or(PipelineError::CounterExhausted)?;
                self.revision = 0;
                self.last_text.clear();
            }
        }
        Ok((!updates.is_empty()).then_some(updates))
    }
}

impl<R: StreamingRecognizer + 'static> AudioProcessor for StreamingProcessor<R> {
    type Output = Vec<CaptionUpdate>;

    fn process(
        &mut self,
        audio: &Message<Box<[f32]>>,
        cancellation: &Cancellation,
    ) -> Result<Option<Self::Output>, PipelineError> {
        if cancellation.is_cancelled() {
            return Ok(None);
        }
        let results = self
            .recognizer
            .push(&audio.value)
            .map_err(|_| PipelineError::ProcessorFailed)?;
        if cancellation.is_cancelled() {
            return Ok(None);
        }
        self.updates(results)
    }

    fn finish(
        &mut self,
        cancellation: &Cancellation,
    ) -> Result<Option<Self::Output>, PipelineError> {
        if cancellation.is_cancelled() {
            return Ok(None);
        }
        let results = self
            .recognizer
            .finish()
            .map_err(|_| PipelineError::ProcessorFailed)?;
        self.updates(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(text: &str, is_final: bool) -> SpeechResult {
        SpeechResult {
            text: text.into(),
            is_final,
            language: is_final.then(|| "en-US".to_owned()),
        }
    }

    fn message(samples: usize) -> Message<Box<[f32]>> {
        Message {
            session_id: 1,
            sequence: 1,
            discontinuity: false,
            value: vec![0.1; samples].into_boxed_slice(),
        }
    }

    fn processor(
        recognizer: MockStreamingRecognizer,
    ) -> StreamingProcessor<MockStreamingRecognizer> {
        StreamingProcessor::new(7, CaptionMode::Auto, recognizer)
    }

    #[test]
    fn interim_results_revise_one_segment_and_a_final_starts_the_next() {
        let mut recognizer = MockStreamingRecognizer::new();
        let mut batches = vec![
            vec![result("And so", false)],
            vec![result("And so", false), result("And so my fellow", false)],
            vec![result("And so my fellow Americans.", true)],
            vec![result("Ask not", false)],
        ]
        .into_iter();
        recognizer
            .expect_push()
            .returning(move |_| Ok(batches.next().unwrap_or_default()));
        let mut processor = processor(recognizer);
        let cancel = Cancellation::default();

        let first = processor
            .process(&message(2_048), &cancel)
            .unwrap()
            .unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!((first[0].segment_id, first[0].revision), (1, 1));
        assert!(!first[0].is_final);

        // A repeated interim text is not sent again.
        let second = processor
            .process(&message(2_048), &cancel)
            .unwrap()
            .unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].text, "And so my fellow");
        assert_eq!(second[0].revision, 2);

        let last = processor
            .process(&message(2_048), &cancel)
            .unwrap()
            .unwrap();
        assert!(last[0].is_final);
        assert_eq!((last[0].segment_id, last[0].revision), (1, 3));
        // The detected language reaches the UI only when the model commits the utterance.
        assert_eq!(first[0].language, None);
        assert_eq!(last[0].language.as_deref(), Some("en-US"));

        let next = processor
            .process(&message(2_048), &cancel)
            .unwrap()
            .unwrap();
        assert_eq!((next[0].segment_id, next[0].revision), (2, 1));
        assert_eq!(next[0].session_id, 7);
    }

    #[test]
    fn silence_and_empty_results_produce_no_captions() {
        let mut recognizer = MockStreamingRecognizer::new();
        recognizer
            .expect_push()
            .returning(|_| Ok(vec![result("", false), result("   ", true)]));
        let mut processor = processor(recognizer);
        let cancel = Cancellation::default();
        assert_eq!(processor.process(&message(2_048), &cancel).unwrap(), None);
        // The empty final still closed its segment.
        assert_eq!(processor.segment_id, 2);
    }

    #[test]
    fn finish_flushes_the_last_utterance_and_cancellation_skips_work() {
        let mut recognizer = MockStreamingRecognizer::new();
        recognizer
            .expect_finish()
            .times(1)
            .returning(|| Ok(vec![result("Thank you.", true)]));
        recognizer.expect_push().times(0);
        let mut processor = processor(recognizer);

        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert_eq!(
            processor.process(&message(2_048), &cancelled).unwrap(),
            None
        );
        assert_eq!(processor.finish(&cancelled).unwrap(), None);

        let flushed = processor.finish(&Cancellation::default()).unwrap().unwrap();
        assert_eq!(flushed[0].text, "Thank you.");
        assert!(flushed[0].is_final);
    }

    #[test]
    fn engine_failures_and_oversized_text_fail_the_processor() {
        let mut recognizer = MockStreamingRecognizer::new();
        let mut calls = 0;
        recognizer.expect_push().returning(move |_| {
            calls += 1;
            if calls == 1 {
                Err(RecognizerError::RecognitionFailed)
            } else {
                Ok(vec![result(&"a".repeat(MAX_CAPTION_CHARS + 1), false)])
            }
        });
        let mut processor = processor(recognizer);
        let cancel = Cancellation::default();
        assert_eq!(
            processor.process(&message(2_048), &cancel),
            Err(PipelineError::ProcessorFailed)
        );
        assert_eq!(
            processor.process(&message(2_048), &cancel),
            Err(PipelineError::ProcessorFailed)
        );
    }
}
