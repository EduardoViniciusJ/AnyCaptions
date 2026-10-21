pub mod consolidator;
pub mod profile;
pub mod session;
pub mod whisper;

use crate::{
    domain::CaptionMode,
    ipc::CaptionUpdate,
    pipeline::{
        AudioProcessor, Cancellation, Message, PipelineError,
        segmenter::SpeechSegmenter,
        vad::{VadFrameAdapter, VoiceActivityDetector},
    },
};
use consolidator::CaptionConsolidator;
use whisper::{MIN_TRANSCRIBE_SAMPLES, SpeechTranscriber, WhisperAdapter};

pub struct TranscriptionProcessor<D: VoiceActivityDetector, T: SpeechTranscriber> {
    session_id: u32,
    vad: VadFrameAdapter<D>,
    segmenter: SpeechSegmenter,
    whisper: WhisperAdapter<T>,
    consolidator: CaptionConsolidator,
}

impl<D: VoiceActivityDetector, T: SpeechTranscriber> TranscriptionProcessor<D, T> {
    pub fn new(
        session_id: u32,
        mode: CaptionMode,
        detector: D,
        segmenter: SpeechSegmenter,
        transcriber: T,
    ) -> Self {
        Self {
            session_id,
            vad: VadFrameAdapter::new(detector),
            segmenter,
            whisper: WhisperAdapter::new(transcriber),
            consolidator: CaptionConsolidator::new(session_id, mode),
        }
    }

    fn finalize_segment(
        &mut self,
        segment: crate::pipeline::segmenter::SpeechSegment,
    ) -> Result<Option<CaptionUpdate>, PipelineError> {
        if segment.samples.len() < MIN_TRANSCRIBE_SAMPLES {
            return Ok(None);
        }
        let Some(text) = self
            .whisper
            .transcribe(&segment.samples)
            .map_err(|_| PipelineError::ProcessorFailed)?
        else {
            return Ok(None);
        };
        Ok(self.consolidator.push(
            self.session_id,
            segment.segment_id,
            segment.revision,
            &text,
            segment.overlap_samples > 0,
            segment.is_final,
        ))
    }
}

impl<D, T> AudioProcessor for TranscriptionProcessor<D, T>
where
    D: VoiceActivityDetector + 'static,
    T: SpeechTranscriber + 'static,
{
    type Output = Vec<CaptionUpdate>;

    fn process(
        &mut self,
        audio: &Message<Box<[f32]>>,
        cancellation: &Cancellation,
    ) -> Result<Option<Self::Output>, PipelineError> {
        if cancellation.is_cancelled() {
            return Ok(None);
        }
        let mut segments = Vec::new();
        if audio.discontinuity
            && let Some(flushed) = self.segmenter.flush()
        {
            segments.push(flushed);
        }
        let frames = self
            .vad
            .push(&audio.value, audio.discontinuity)
            .map_err(|_| PipelineError::ProcessorFailed)?;

        for evaluated in frames {
            if cancellation.is_cancelled() {
                return Ok(None);
            }
            segments.extend(
                self.segmenter
                    .push_frame(&evaluated.samples, evaluated.speech_probability),
            );
        }

        let last_partial_index = segments.iter().rposition(|seg| !seg.is_final);
        let has_final_for_same_segment = |segment_id: u32| {
            segments
                .iter()
                .any(|seg| seg.is_final && seg.segment_id == segment_id)
        };

        let mut updates = Vec::new();
        for (idx, segment) in segments.iter().enumerate() {
            if cancellation.is_cancelled() {
                return Ok(None);
            }
            if !segment.is_final
                && (Some(idx) != last_partial_index
                    || has_final_for_same_segment(segment.segment_id))
            {
                continue;
            }
            if segment.samples.len() < MIN_TRANSCRIBE_SAMPLES {
                continue;
            }
            let maybe_text = self
                .whisper
                .transcribe(&segment.samples)
                .map_err(|_| PipelineError::ProcessorFailed)?;
            if cancellation.is_cancelled() {
                return Ok(None);
            }
            if let Some(text) = maybe_text
                && let Some(update) = self.consolidator.push(
                    self.session_id,
                    segment.segment_id,
                    segment.revision,
                    &text,
                    segment.overlap_samples > 0,
                    segment.is_final,
                )
            {
                updates.push(update);
            }
        }
        Ok((!updates.is_empty()).then_some(updates))
    }

    fn finish(
        &mut self,
        cancellation: &Cancellation,
    ) -> Result<Option<Self::Output>, PipelineError> {
        if cancellation.is_cancelled() {
            return Ok(None);
        }
        let Some(segment) = self.segmenter.flush() else {
            return Ok(None);
        };
        Ok(self.finalize_segment(segment)?.map(|update| vec![update]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::{
        Pipeline,
        segmenter::SegmenterConfig,
        vad::{MockVoiceActivityDetector, VAD_FRAME_SAMPLES},
    };
    use std::time::{Duration, Instant};
    use whisper::MockSpeechTranscriber;

    fn test_segmenter() -> SpeechSegmenter {
        SpeechSegmenter::new(SegmenterConfig {
            speech_threshold: 0.5,
            silence_threshold: 0.35,
            pre_roll_frames: 1,
            min_speech_frames: 3,
            hangover_frames: 2,
            partial_interval_frames: 4,
            max_segment_frames: 10,
            overlap_frames: 2,
        })
        .unwrap()
    }

    #[test]
    fn pipeline_processes_voiced_audio_into_partial_and_final_captions_and_ignores_silence() {
        let mut vad = MockVoiceActivityDetector::new();
        vad.expect_predict_frame()
            .returning(|frame| Ok(if frame[0].abs() > 0.1 { 0.9 } else { 0.05 }));
        vad.expect_reset().return_const(());

        let mut transcriber = MockSpeechTranscriber::new();
        transcriber.expect_transcribe_raw().returning(|samples| {
            if samples.len() <= 5 * VAD_FRAME_SAMPLES {
                Ok("Hello from".into())
            } else {
                Ok("Hello from AnyCaptions!".into())
            }
        });

        let processor =
            TranscriptionProcessor::new(7, CaptionMode::EnEn, vad, test_segmenter(), transcriber);
        let mut pipeline = Pipeline::default();
        let session = pipeline.start(processor).unwrap();

        // 4 voiced frames (2048 samples >= 1600 MIN_TRANSCRIBE_SAMPLES) trigger a partial caption.
        pipeline
            .submit(session, &vec![0.25; 4 * VAD_FRAME_SAMPLES], false)
            .unwrap();
        // 2 silent frames trigger hangover completion and a final caption.
        pipeline
            .submit(session, &vec![0.0; 2 * VAD_FRAME_SAMPLES], false)
            .unwrap();

        let mut collected = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while collected.len() < 2 && Instant::now() < deadline {
            if let Some(msg) = pipeline.try_result().unwrap() {
                collected.extend(msg.value);
            } else {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        pipeline.stop().unwrap();

        assert_eq!(collected.len(), 2);
        assert_eq!(collected[0].session_id, 7);
        assert_eq!(collected[0].segment_id, 1);
        assert!(!collected[0].is_final);
        assert_eq!(collected[0].text, "Hello from");

        assert_eq!(collected[1].session_id, 7);
        assert_eq!(collected[1].segment_id, 1);
        assert!(collected[1].is_final);
        assert_eq!(collected[1].text, "Hello from AnyCaptions!");
    }

    #[test]
    fn graceful_finish_emits_the_active_speech_as_a_final_caption() {
        let mut vad = MockVoiceActivityDetector::new();
        vad.expect_predict_frame().returning(|_| Ok(0.9));
        vad.expect_reset().return_const(());

        let mut transcriber = MockSpeechTranscriber::new();
        transcriber
            .expect_transcribe_raw()
            .returning(|_| Ok("Thank you for listening.".into()));

        let processor =
            TranscriptionProcessor::new(9, CaptionMode::EnEn, vad, test_segmenter(), transcriber);
        let mut pipeline = Pipeline::default();
        let session = pipeline.start(processor).unwrap();
        pipeline
            .submit(session, &vec![0.25; 4 * VAD_FRAME_SAMPLES], false)
            .unwrap();

        let results = pipeline.finish().unwrap();
        assert!(results.iter().any(|message| {
            message
                .value
                .iter()
                .any(|update| update.is_final && update.text == "Thank you for listening.")
        }));
    }
}
