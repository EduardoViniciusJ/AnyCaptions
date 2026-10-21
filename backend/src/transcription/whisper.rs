use std::path::Path;

use super::{
    consolidator::sanitize_whisper_text,
    profile::{ModelProfileError, WhisperModelProfile},
};

pub const MIN_TRANSCRIBE_SAMPLES: usize = 1_600; // 100 ms at 16 kHz
pub const MAX_TRANSCRIBE_SAMPLES: usize = 30 * 16_000; // 30 s at 16 kHz
pub const MAX_TRANSCRIPT_CHARS: usize = 4_000;
const SILENCE_RMS_FLOOR: f32 = 0.0005;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptionError {
    ModelMissing,
    ModelCorrupted,
    ModelIncompatible,
    InvalidAudio,
    InferenceFailed,
}

impl From<ModelProfileError> for TranscriptionError {
    fn from(value: ModelProfileError) -> Self {
        match value {
            ModelProfileError::Missing => Self::ModelMissing,
            ModelProfileError::Corrupted => Self::ModelCorrupted,
            ModelProfileError::Incompatible => Self::ModelIncompatible,
        }
    }
}

#[cfg_attr(test, mockall::automock)]
pub trait SpeechTranscriber: Send {
    fn transcribe_raw(&mut self, samples: &[f32]) -> Result<String, TranscriptionError>;
}

pub struct WhisperAdapter<T: SpeechTranscriber> {
    engine: T,
}

impl<T: SpeechTranscriber> WhisperAdapter<T> {
    pub fn new(engine: T) -> Self {
        Self { engine }
    }

    pub fn transcribe(&mut self, samples: &[f32]) -> Result<Option<String>, TranscriptionError> {
        if samples.len() < MIN_TRANSCRIBE_SAMPLES
            || samples.len() > MAX_TRANSCRIBE_SAMPLES
            || samples.iter().any(|sample| !sample.is_finite())
        {
            return Err(TranscriptionError::InvalidAudio);
        }
        if is_near_digital_silence(samples) {
            return Ok(None);
        }
        let raw = self.engine.transcribe_raw(samples)?;
        let cleaned = sanitize_whisper_text(&raw);
        if cleaned.is_empty() || is_non_speech_marker(&cleaned) {
            return Ok(None);
        }
        if cleaned.len() > MAX_TRANSCRIPT_CHARS {
            return Err(TranscriptionError::InferenceFailed);
        }
        Ok(Some(cleaned))
    }
}

fn is_near_digital_silence(samples: &[f32]) -> bool {
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    let rms = (sum_sq / samples.len() as f64).sqrt() as f32;
    rms < SILENCE_RMS_FLOOR
}

fn is_non_speech_marker(text: &str) -> bool {
    matches!(text.trim().to_ascii_lowercase().as_str(), "." | "...")
}

#[cfg(feature = "whisper")]
pub struct NativeWhisperTranscriber {
    _context: whisper_rs::WhisperContext,
    state: whisper_rs::WhisperState,
    threads: i32,
    audio_buf: Vec<f32>,
    last_prompt: String,
}

#[cfg(feature = "whisper")]
impl NativeWhisperTranscriber {
    pub fn load(path: &Path, profile: &WhisperModelProfile) -> Result<Self, TranscriptionError> {
        profile.verify(path)?;
        whisper_rs::install_logging_hooks();
        let mut context_params = whisper_rs::WhisperContextParameters::default();
        context_params.use_gpu(profile.use_gpu);
        context_params.flash_attn(profile.use_flash_attn);
        let context = whisper_rs::WhisperContext::new_with_params(path, context_params)
            .map_err(|_| TranscriptionError::ModelIncompatible)?;
        let state = context
            .create_state()
            .map_err(|_| TranscriptionError::ModelIncompatible)?;
        Ok(Self {
            _context: context,
            state,
            threads: profile.threads.clamp(1, 16),
            audio_buf: Vec::with_capacity(16_000 * 6),
            last_prompt: String::new(),
        })
    }
}

#[cfg(feature = "whisper")]
impl SpeechTranscriber for NativeWhisperTranscriber {
    fn transcribe_raw(&mut self, samples: &[f32]) -> Result<String, TranscriptionError> {
        use whisper_rs::{FullParams, SamplingStrategy};

        // 1. Automatic peak normalization for quiet desktop/game/Discord audio + 200 ms trailing pad
        let peak = samples
            .iter()
            .fold(0.0_f32, |max, &s| max.max(s.abs()));
        let gain = if peak > 0.002 && peak < 0.80 {
            (0.85 / peak).min(8.0)
        } else {
            1.0
        };
        self.audio_buf.clear();
        self.audio_buf.reserve(samples.len() + 3_200);
        for &s in samples {
            self.audio_buf.push((s * gain).clamp(-1.0, 1.0));
        }
        // 200 ms trailing zero-pad prevents Whisper from clipping the final word of live partials
        self.audio_buf.resize(samples.len() + 3_200, 0.0);

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_max_tokens(0);
        params.set_n_threads(self.threads);
        params.set_language(Some("en"));
        params.set_detect_language(false);
        params.set_translate(false);
        params.set_no_context(true);
        if !self.last_prompt.is_empty() {
            params.set_initial_prompt(&self.last_prompt);
        }
        params.set_no_timestamps(true);
        params.set_single_segment(true);
        params.set_suppress_blank(true);
        params.set_suppress_nst(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_temperature(0.0);
        params.set_temperature_inc(0.0);

        self.state
            .full(params, &self.audio_buf)
            .map_err(|_| TranscriptionError::InferenceFailed)?;
        let mut output = String::new();
        for segment in self.state.as_iter() {
            let piece = segment
                .to_str()
                .map_err(|_| TranscriptionError::InferenceFailed)?;
            if output.len() + piece.len() > MAX_TRANSCRIPT_CHARS {
                return Err(TranscriptionError::InferenceFailed);
            }
            output.push_str(piece);
        }
        let cleaned = sanitize_whisper_text(&output);
        if !cleaned.is_empty() && !is_non_speech_marker(&cleaned) {
            let words: Vec<&str> = cleaned.split_whitespace().collect();
            let start = words.len().saturating_sub(12);
            self.last_prompt = words[start..].join(" ");
        }
        Ok(output)
    }
}

pub fn verify_whisper_profile(
    path: &Path,
    profile: &WhisperModelProfile,
) -> Result<(), TranscriptionError> {
    profile.verify(path).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voiced_samples(count: usize) -> Vec<f32> {
        (0..count)
            .map(|i| if i % 2 == 0 { 0.25 } else { -0.25 })
            .collect()
    }

    #[test]
    fn silence_and_non_verbal_outputs_produce_no_caption_without_calling_engine_on_silence() {
        let mut mock = MockSpeechTranscriber::new();
        mock.expect_transcribe_raw()
            .times(1)
            .returning(|_| Ok("[BLANK_AUDIO] (music)".into()));

        let mut adapter = WhisperAdapter::new(mock);
        // Digital silence returns None without invoking the engine.
        assert_eq!(adapter.transcribe(&vec![0.0; 3_200]), Ok(None));
        // Non-verbal markers from the engine also return None.
        assert_eq!(adapter.transcribe(&voiced_samples(3_200)), Ok(None));
    }

    #[test]
    fn valid_speech_returns_cleaned_english_text_and_validates_bounds() {
        let mut mock = MockSpeechTranscriber::new();
        mock.expect_transcribe_raw()
            .times(1)
            .returning(|_| Ok("  Hello from AnyCaptions! [music] ".into()));

        let mut adapter = WhisperAdapter::new(mock);
        assert_eq!(
            adapter.transcribe(&voiced_samples(100)),
            Err(TranscriptionError::InvalidAudio)
        );
        assert_eq!(
            adapter.transcribe(&[f32::NAN; 2_000]),
            Err(TranscriptionError::InvalidAudio)
        );
        assert_eq!(
            adapter.transcribe(&voiced_samples(3_200)),
            Ok(Some("Hello from AnyCaptions!".into()))
        );
    }

    #[test]
    fn preserves_short_valid_utterances_that_can_look_like_hallucinations() {
        let mut mock = MockSpeechTranscriber::new();
        mock.expect_transcribe_raw()
            .returning(|_| Ok("Thank you.".into()));

        let mut adapter = WhisperAdapter::new(mock);
        assert_eq!(
            adapter.transcribe(&voiced_samples(3_200)),
            Ok(Some("Thank you.".into()))
        );
    }
}
