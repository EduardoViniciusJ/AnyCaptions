use std::collections::VecDeque;

use super::vad::VAD_FRAME_SAMPLES;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SegmenterConfig {
    pub speech_threshold: f32,
    pub silence_threshold: f32,
    pub pre_roll_frames: usize,
    pub min_speech_frames: usize,
    pub hangover_frames: usize,
    pub partial_interval_frames: usize,
    pub max_segment_frames: usize,
    pub overlap_frames: usize,
}

impl Default for SegmenterConfig {
    fn default() -> Self {
        Self {
            speech_threshold: 0.45,
            silence_threshold: 0.32,
            pre_roll_frames: 6,          // 192 ms pre-roll before speech onset
            min_speech_frames: 6,        // 192 ms minimum speech
            hangover_frames: 14,         // 448 ms pause keeps the full spoken line together
            partial_interval_frames: 12, // 384 ms snapshots; live captions transcribe finals only
            max_segment_frames: 188,     // 6 s; natural pauses usually end segments first
            overlap_frames: 6,           // 192 ms boundary overlap
        }
    }
}

impl SegmenterConfig {
    pub fn validate(&self) -> bool {
        self.speech_threshold.is_finite()
            && self.silence_threshold.is_finite()
            && (0.0..=1.0).contains(&self.speech_threshold)
            && (0.0..=self.speech_threshold).contains(&self.silence_threshold)
            && self.min_speech_frames >= 1
            && self.partial_interval_frames >= 1
            && self.max_segment_frames > self.pre_roll_frames + self.overlap_frames
            && self.max_segment_frames <= 625 // At most 20 seconds (625 * 32 ms)
            && self.overlap_frames < self.max_segment_frames
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeechSegment {
    pub segment_id: u32,
    pub revision: u32,
    pub samples: Vec<f32>,
    pub overlap_samples: usize,
    pub is_final: bool,
}

pub struct SpeechSegmenter {
    config: SegmenterConfig,
    pre_roll: VecDeque<[f32; VAD_FRAME_SAMPLES]>,
    active_frames: Vec<[f32; VAD_FRAME_SAMPLES]>,
    voiced_frames: usize,
    trailing_silence: usize,
    frames_since_partial: usize,
    overlap_frames_in_current: usize,
    next_segment_id: u32,
    current_revision: u32,
    in_speech: bool,
}

impl SpeechSegmenter {
    pub fn new(config: SegmenterConfig) -> Option<Self> {
        if !config.validate() {
            return None;
        }
        Some(Self {
            pre_roll: VecDeque::with_capacity(config.pre_roll_frames.max(1)),
            active_frames: Vec::with_capacity(config.max_segment_frames),
            config,
            voiced_frames: 0,
            trailing_silence: 0,
            frames_since_partial: 0,
            overlap_frames_in_current: 0,
            next_segment_id: 1,
            current_revision: 0,
            in_speech: false,
        })
    }

    pub fn push_frame(
        &mut self,
        frame: &[f32; VAD_FRAME_SAMPLES],
        speech_probability: f32,
    ) -> Vec<SpeechSegment> {
        let mut emitted = Vec::new();
        if !self.in_speech {
            if speech_probability >= self.config.speech_threshold {
                self.in_speech = true;
                self.voiced_frames = 1;
                self.trailing_silence = 0;
                self.frames_since_partial = 1;
                self.current_revision = 0;
                self.overlap_frames_in_current = 0;
                self.active_frames.clear();
                self.active_frames.extend(self.pre_roll.drain(..));
                self.active_frames.push(*frame);
            } else {
                self.push_pre_roll(*frame);
            }
            return emitted;
        }

        self.active_frames.push(*frame);
        if speech_probability >= self.config.silence_threshold {
            self.voiced_frames += 1;
            self.trailing_silence = 0;
        } else {
            self.trailing_silence += 1;
        }
        self.frames_since_partial += 1;

        if self.trailing_silence >= self.config.hangover_frames {
            if let Some(segment) = self.finalize_active(false) {
                emitted.push(segment);
            }
            return emitted;
        }

        if self.active_frames.len() >= self.config.max_segment_frames {
            if let Some(segment) = self.finalize_active(true) {
                emitted.push(segment);
            }
            return emitted;
        }

        if self.voiced_frames >= self.config.min_speech_frames
            && self.frames_since_partial >= self.config.partial_interval_frames
        {
            self.frames_since_partial = 0;
            if let Some(partial) = self.build_segment(false) {
                emitted.push(partial);
            }
        }

        emitted
    }

    /// Explicit policy on capture discontinuity or end of stream:
    /// emits any valid buffered speech as final and resets internal buffers.
    pub fn flush(&mut self) -> Option<SpeechSegment> {
        let segment = if self.in_speech {
            self.finalize_active(false)
        } else {
            None
        };
        self.pre_roll.clear();
        self.active_frames.clear();
        self.in_speech = false;
        self.voiced_frames = 0;
        self.trailing_silence = 0;
        self.frames_since_partial = 0;
        self.overlap_frames_in_current = 0;
        segment
    }

    pub fn reset_session(&mut self) {
        let _ = self.flush();
        self.next_segment_id = 1;
        self.current_revision = 0;
    }

    fn push_pre_roll(&mut self, frame: [f32; VAD_FRAME_SAMPLES]) {
        if self.config.pre_roll_frames == 0 {
            return;
        }
        if self.pre_roll.len() == self.config.pre_roll_frames {
            self.pre_roll.pop_front();
        }
        self.pre_roll.push_back(frame);
    }

    fn build_segment(&mut self, is_final: bool) -> Option<SpeechSegment> {
        self.current_revision = self.current_revision.checked_add(1)?;
        let mut samples = Vec::with_capacity(self.active_frames.len() * VAD_FRAME_SAMPLES);
        for frame in &self.active_frames {
            samples.extend_from_slice(frame);
        }
        Some(SpeechSegment {
            segment_id: self.next_segment_id,
            revision: self.current_revision,
            samples,
            overlap_samples: self.overlap_frames_in_current * VAD_FRAME_SAMPLES,
            is_final,
        })
    }

    fn finalize_active(&mut self, split_by_max_duration: bool) -> Option<SpeechSegment> {
        let valid = self.voiced_frames >= self.config.min_speech_frames;
        let segment = if valid {
            self.build_segment(true)
        } else {
            None
        };
        if valid {
            self.next_segment_id = self.next_segment_id.saturating_add(1);
        }
        if split_by_max_duration && valid && self.config.overlap_frames > 0 {
            let keep = self.config.overlap_frames.min(self.active_frames.len());
            let tail = self.active_frames[self.active_frames.len() - keep..].to_vec();
            self.active_frames = tail;
            self.overlap_frames_in_current = keep;
            self.voiced_frames = self.config.min_speech_frames;
            self.trailing_silence = 0;
            self.frames_since_partial = 0;
            self.current_revision = 0;
            self.in_speech = true;
        } else {
            self.active_frames.clear();
            self.overlap_frames_in_current = 0;
            self.voiced_frames = 0;
            self.trailing_silence = 0;
            self.frames_since_partial = 0;
            self.current_revision = 0;
            self.in_speech = false;
        }
        segment
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(value: f32) -> [f32; VAD_FRAME_SAMPLES] {
        [value; VAD_FRAME_SAMPLES]
    }

    fn test_config() -> SegmenterConfig {
        SegmenterConfig {
            speech_threshold: 0.5,
            silence_threshold: 0.35,
            pre_roll_frames: 2,
            min_speech_frames: 2,
            hangover_frames: 3,
            partial_interval_frames: 3,
            max_segment_frames: 8,
            overlap_frames: 2,
        }
    }

    #[test]
    fn prolonged_silence_produces_no_segments_and_keeps_bounded_pre_roll() {
        let mut segmenter = SpeechSegmenter::new(test_config()).unwrap();
        for i in 0..100 {
            assert!(segmenter.push_frame(&frame(i as f32), 0.1).is_empty());
        }
        assert_eq!(segmenter.pre_roll.len(), 2);
        assert!(segmenter.flush().is_none());
    }

    #[test]
    fn preserves_word_onset_with_pre_roll_and_tolerates_brief_pauses() {
        let mut segmenter = SpeechSegmenter::new(test_config()).unwrap();
        segmenter.push_frame(&frame(1.0), 0.1);
        segmenter.push_frame(&frame(2.0), 0.1);
        // Speech starts on frame 3; pre-roll frames 1 and 2 must be included.
        assert!(segmenter.push_frame(&frame(3.0), 0.8).is_empty());
        // Brief 2-frame pause (< hangover_frames = 3) does not split the phrase.
        assert!(segmenter.push_frame(&frame(4.0), 0.1).is_empty());
        let partial = segmenter.push_frame(&frame(5.0), 0.9);
        assert_eq!(partial.len(), 1);
        assert!(!partial[0].is_final);
        assert_eq!(partial[0].segment_id, 1);
        assert_eq!(partial[0].revision, 1);
        assert_eq!(partial[0].samples[0], 1.0);

        // Trailing silence reaches hangover_frames (3 frames) and finalizes segment 1.
        assert!(segmenter.push_frame(&frame(6.0), 0.0).is_empty());
        assert!(segmenter.push_frame(&frame(7.0), 0.0).is_empty());
        let finalized = segmenter.push_frame(&frame(8.0), 0.0);
        assert_eq!(finalized.len(), 1);
        assert!(finalized[0].is_final);
        assert_eq!(finalized[0].segment_id, 1);
        assert_eq!(finalized[0].revision, 2);
        assert_eq!(finalized[0].samples.len(), 8 * VAD_FRAME_SAMPLES);
    }

    #[test]
    fn continuous_speech_splits_at_max_duration_with_overlap_context() {
        let mut segmenter = SpeechSegmenter::new(test_config()).unwrap();
        let mut finals = Vec::new();
        for step in 1..=12 {
            for seg in segmenter.push_frame(&frame(step as f32), 0.9) {
                if seg.is_final {
                    finals.push(seg);
                }
            }
        }
        if let Some(last) = segmenter.flush() {
            finals.push(last);
        }
        assert_eq!(finals.len(), 2);
        assert_eq!(finals[0].segment_id, 1);
        assert_eq!(finals[0].overlap_samples, 0);
        assert_eq!(finals[0].samples.len(), 8 * VAD_FRAME_SAMPLES);
        assert_eq!(finals[1].segment_id, 2);
        assert_eq!(finals[1].overlap_samples, 2 * VAD_FRAME_SAMPLES);
        // The first two frames of segment 2 must match the last two frames of segment 1 (7.0 and 8.0).
        assert_eq!(finals[1].samples[0], 7.0);
        assert_eq!(finals[1].samples[VAD_FRAME_SAMPLES], 8.0);
    }

    #[test]
    fn discontinuity_flushes_active_speech_and_discards_short_noise_blips() {
        let mut segmenter = SpeechSegmenter::new(test_config()).unwrap();
        // Single noise frame (< min_speech_frames = 2) followed by flush is discarded.
        segmenter.push_frame(&frame(1.0), 0.9);
        assert!(segmenter.flush().is_none());

        // Two voiced frames followed by discontinuity/stop flush are preserved.
        segmenter.push_frame(&frame(2.0), 0.9);
        segmenter.push_frame(&frame(3.0), 0.9);
        let flushed = segmenter.flush().unwrap();
        assert!(flushed.is_final);
        assert_eq!(flushed.segment_id, 1);
        assert_eq!(flushed.samples.len(), 2 * VAD_FRAME_SAMPLES);
    }
}
