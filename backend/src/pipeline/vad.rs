use std::{fs::File, io::Read, path::Path};

use ort::{
    session::{Session, builder::GraphOptimizationLevel},
    value::TensorRef,
};
use sha2::{Digest, Sha256};

pub const VAD_SAMPLE_RATE: i64 = 16_000;
pub const VAD_FRAME_SAMPLES: usize = 512;
const SILERO_CONTEXT_SAMPLES: usize = 64;
const SILERO_INPUT_SAMPLES: usize = SILERO_CONTEXT_SAMPLES + VAD_FRAME_SAMPLES;
const SILERO_STATE_LEN: usize = 2 * 128;
const MAX_MODEL_BYTES: usize = 16 * 1024 * 1024;
const MAX_CHUNK_SAMPLES: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VadError {
    ModelMissing,
    ModelCorrupted,
    ModelIncompatible,
    InvalidInput,
    InferenceFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VadModelProfile {
    pub id: &'static str,
    pub revision: &'static str,
    pub runtime: &'static str,
    pub source: &'static str,
    pub license: &'static str,
    pub sha256: &'static str,
}

impl VadModelProfile {
    pub const fn silero_v5() -> Self {
        Self {
            id: "silero-vad-v5",
            revision: "6478567951ae5c9979ad7b234185b5515f4be7a1",
            runtime: "onnxruntime-cpu",
            source: "https://github.com/snakers4/silero-vad",
            license: "MIT",
            sha256: "2623a2953f6ff3d2c1e61740c6cdb7168133479b267dfef114a4a3cc5bdd788f",
        }
    }

    pub fn verify_and_read(&self, path: &Path) -> Result<Vec<u8>, VadError> {
        let file = File::open(path).map_err(|_| VadError::ModelMissing)?;
        let mut bytes = Vec::new();
        file.take(MAX_MODEL_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| VadError::ModelCorrupted)?;
        if bytes.is_empty() || bytes.len() > MAX_MODEL_BYTES {
            return Err(VadError::ModelCorrupted);
        }
        let digest: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if !digest.eq_ignore_ascii_case(self.sha256) {
            return Err(VadError::ModelCorrupted);
        }
        Ok(bytes)
    }
}

#[cfg_attr(test, mockall::automock)]
pub trait VoiceActivityDetector: Send {
    fn predict_frame(&mut self, frame: &[f32; VAD_FRAME_SAMPLES]) -> Result<f32, VadError>;
    fn reset(&mut self);
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedFrame {
    pub samples: [f32; VAD_FRAME_SAMPLES],
    pub speech_probability: f32,
}

pub struct VadFrameAdapter<D: VoiceActivityDetector> {
    detector: D,
    pending: [f32; VAD_FRAME_SAMPLES],
    filled: usize,
}

impl<D: VoiceActivityDetector> VadFrameAdapter<D> {
    pub fn new(detector: D) -> Self {
        Self {
            detector,
            pending: [0.0; VAD_FRAME_SAMPLES],
            filled: 0,
        }
    }

    pub fn reset(&mut self) {
        self.filled = 0;
        self.detector.reset();
    }

    pub fn push(
        &mut self,
        samples: &[f32],
        discontinuity: bool,
    ) -> Result<Vec<EvaluatedFrame>, VadError> {
        if samples.len() > MAX_CHUNK_SAMPLES || samples.iter().any(|sample| !sample.is_finite()) {
            return Err(VadError::InvalidInput);
        }
        if discontinuity {
            self.reset();
        }
        let mut out = Vec::with_capacity((self.filled + samples.len()) / VAD_FRAME_SAMPLES);
        let mut remaining = samples;
        while !remaining.is_empty() {
            let take = (VAD_FRAME_SAMPLES - self.filled).min(remaining.len());
            self.pending[self.filled..self.filled + take].copy_from_slice(&remaining[..take]);
            self.filled += take;
            remaining = &remaining[take..];
            if self.filled == VAD_FRAME_SAMPLES {
                let frame = self.pending;
                self.filled = 0;
                let probability = self.detector.predict_frame(&frame)?;
                if !probability.is_finite() || !(0.0..=1.0).contains(&probability) {
                    return Err(VadError::InferenceFailed);
                }
                out.push(EvaluatedFrame {
                    samples: frame,
                    speech_probability: probability,
                });
            }
        }
        Ok(out)
    }
}

pub struct SileroVad {
    session: Session,
    state: [f32; SILERO_STATE_LEN],
    context: [f32; SILERO_CONTEXT_SAMPLES],
}

impl SileroVad {
    pub fn load(path: &Path, profile: &VadModelProfile) -> Result<Self, VadError> {
        let bytes = profile.verify_and_read(path)?;
        let session = Session::builder()
            .map_err(|_| VadError::ModelIncompatible)?
            .with_optimization_level(GraphOptimizationLevel::Level3)
            .map_err(|_| VadError::ModelIncompatible)?
            .with_intra_threads(1)
            .map_err(|_| VadError::ModelIncompatible)?
            .with_inter_threads(1)
            .map_err(|_| VadError::ModelIncompatible)?
            .commit_from_memory(&bytes)
            .map_err(|_| VadError::ModelIncompatible)?;

        let mut vad = Self {
            session,
            state: [0.0; SILERO_STATE_LEN],
            context: [0.0; SILERO_CONTEXT_SAMPLES],
        };
        // Probe once with silence to verify input/output tensor signatures before use.
        vad.predict_frame(&[0.0; VAD_FRAME_SAMPLES])
            .map_err(|_| VadError::ModelIncompatible)?;
        vad.reset();
        Ok(vad)
    }
}

impl VoiceActivityDetector for SileroVad {
    fn predict_frame(&mut self, frame: &[f32; VAD_FRAME_SAMPLES]) -> Result<f32, VadError> {
        if frame.iter().any(|sample| !sample.is_finite()) {
            return Err(VadError::InvalidInput);
        }
        let mut input_buf = [0.0_f32; SILERO_INPUT_SAMPLES];
        input_buf[..SILERO_CONTEXT_SAMPLES].copy_from_slice(&self.context);
        input_buf[SILERO_CONTEXT_SAMPLES..].copy_from_slice(frame);

        let sr = [VAD_SAMPLE_RATE];
        let input_tensor =
            TensorRef::from_array_view(([1_usize, SILERO_INPUT_SAMPLES], &input_buf[..]))
                .map_err(|_| VadError::InferenceFailed)?;
        let state_tensor =
            TensorRef::from_array_view(([2_usize, 1_usize, 128_usize], &self.state[..]))
                .map_err(|_| VadError::InferenceFailed)?;
        let sr_tensor = TensorRef::from_array_view(([1_usize], &sr[..]))
            .map_err(|_| VadError::InferenceFailed)?;

        let outputs = self
            .session
            .run(ort::inputs![
                "input" => input_tensor,
                "state" => state_tensor,
                "sr" => sr_tensor,
            ])
            .map_err(|_| VadError::InferenceFailed)?;

        let (_, prob_slice) = outputs["output"]
            .try_extract_tensor::<f32>()
            .map_err(|_| VadError::ModelIncompatible)?;
        let probability = *prob_slice.first().ok_or(VadError::ModelIncompatible)?;
        if !probability.is_finite() {
            return Err(VadError::InferenceFailed);
        }

        let (_, state_slice) = outputs["stateN"]
            .try_extract_tensor::<f32>()
            .map_err(|_| VadError::ModelIncompatible)?;
        if state_slice.len() != SILERO_STATE_LEN || state_slice.iter().any(|v| !v.is_finite()) {
            return Err(VadError::ModelIncompatible);
        }
        self.state.copy_from_slice(state_slice);
        self.context
            .copy_from_slice(&frame[VAD_FRAME_SAMPLES - SILERO_CONTEXT_SAMPLES..]);
        Ok(probability.clamp(0.0, 1.0))
    }

    fn reset(&mut self) {
        self.state.fill(0.0);
        self.context.fill(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_arbitrary_input_into_512_sample_frames_and_resets_on_discontinuity() {
        let mut mock = MockVoiceActivityDetector::new();
        mock.expect_predict_frame()
            .times(2)
            .returning(|frame| Ok(if frame[0] > 0.0 { 0.85 } else { 0.05 }));
        mock.expect_reset().times(1).return_const(());

        let mut adapter = VadFrameAdapter::new(mock);
        // Push 300 samples: not enough for a full 512-sample frame yet.
        assert!(adapter.push(&vec![1.0; 300], false).unwrap().is_empty());
        // Push 250 more: 512 samples complete a frame, 38 remain buffered.
        let first = adapter.push(&vec![1.0; 250], false).unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].speech_probability, 0.85);

        // Discontinuity clears the 38 buffered samples and resets detector state.
        let second = adapter.push(&vec![0.0; VAD_FRAME_SAMPLES], true).unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].speech_probability, 0.05);
    }

    #[test]
    fn rejects_non_finite_samples_and_invalid_probabilities() {
        let mut mock = MockVoiceActivityDetector::new();
        mock.expect_predict_frame().times(1).returning(|_| Ok(1.5));
        let mut adapter = VadFrameAdapter::new(mock);
        assert_eq!(
            adapter.push(&[f32::NAN], false),
            Err(VadError::InvalidInput)
        );
        assert_eq!(
            adapter.push(&vec![0.0; VAD_FRAME_SAMPLES], false),
            Err(VadError::InferenceFailed)
        );
    }

    #[test]
    fn reports_missing_and_corrupted_model_files_before_loading_ort() {
        let profile = VadModelProfile::silero_v5();
        let missing = std::env::temp_dir().join("anycaptions-missing-silero.onnx");
        assert_eq!(
            SileroVad::load(&missing, &profile).err(),
            Some(VadError::ModelMissing)
        );

        let corrupted = std::env::temp_dir().join(format!(
            "anycaptions-corrupt-silero-{}.onnx",
            std::process::id()
        ));
        std::fs::write(&corrupted, b"not-a-valid-onnx-model").unwrap();
        assert_eq!(
            SileroVad::load(&corrupted, &profile).err(),
            Some(VadError::ModelCorrupted)
        );
        let _ = std::fs::remove_file(corrupted);
    }
}
