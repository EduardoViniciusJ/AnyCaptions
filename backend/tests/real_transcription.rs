use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use anycaptions_lib::{
    domain::CaptionMode,
    pipeline::{
        MAX_BLOCK_SAMPLES, Pipeline,
        segmenter::{SegmenterConfig, SpeechSegmenter},
        vad::{SileroVad, VAD_FRAME_SAMPLES, VadFrameAdapter, VadModelProfile},
    },
    transcription::{
        TranscriptionProcessor, profile::WhisperModelProfile, whisper::NativeWhisperTranscriber,
    },
};

fn backend_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn load_wav_16k_mono(path: &Path) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).expect("fixture wav should exist");
    let spec = reader.spec();
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, 16_000);
    reader
        .samples::<i16>()
        .map(|s| f32::from(s.unwrap()) / 32768.0)
        .collect()
}

#[test]
#[ignore = "requires local silero_vad.onnx and wav fixtures"]
fn silero_vad_distinguishes_real_speech_from_silence_and_synthetic_noise() {
    let root = backend_dir();
    let vad_path = root.join("models/silero_vad.onnx");
    let jfk_path = root.join("benchmarks/local/jfk.wav");
    let silence_path = root.join("benchmarks/local/silence.wav");

    let vad = SileroVad::load(&vad_path, &VadModelProfile::silero_v5())
        .expect("real Silero VAD model should load");
    let mut adapter = VadFrameAdapter::new(vad);

    let silence = load_wav_16k_mono(&silence_path);
    let silence_frames = adapter.push(&silence, true).unwrap();
    let max_silence_prob = silence_frames
        .iter()
        .map(|f| f.speech_probability)
        .fold(0.0_f32, f32::max);
    assert!(
        max_silence_prob < 0.20,
        "expected silence probability < 0.20, got {max_silence_prob}"
    );

    // Deterministic low-amplitude stationary noise (LCG PRNG, no speech formants).
    let mut state = 0x1234_5678_u32;
    let noise: Vec<f32> = (0..16_000 * 2)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let unit = (state as f32 / u32::MAX as f32) * 2.0 - 1.0;
            unit * 0.002
        })
        .collect();
    let noise_frames = adapter.push(&noise, true).unwrap();
    let voiced_noise_count = noise_frames
        .iter()
        .filter(|f| f.speech_probability >= 0.50)
        .count();
    assert_eq!(
        voiced_noise_count, 0,
        "stationary noise should not trigger VAD"
    );

    let speech = load_wav_16k_mono(&jfk_path);
    adapter.reset();
    let mut speech_frames = Vec::new();
    for chunk in speech.chunks(MAX_BLOCK_SAMPLES) {
        speech_frames.extend(adapter.push(chunk, false).unwrap());
    }
    let voiced_speech_count = speech_frames
        .iter()
        .filter(|f| f.speech_probability >= 0.50)
        .count();
    let max_speech_prob = speech_frames
        .iter()
        .map(|f| f.speech_probability)
        .fold(0.0_f32, f32::max);
    assert!(
        voiced_speech_count >= 50 && max_speech_prob > 0.85,
        "expected strong speech detection on jfk.wav, got {voiced_speech_count} voiced frames (max={max_speech_prob})"
    );
}

#[test]
#[ignore = "requires local silero_vad.onnx, ggml-tiny.en.bin, and wav fixtures"]
fn whisper_and_pipeline_transcribe_real_english_speech_and_suppress_silence() {
    let root = backend_dir();
    let vad_path = root.join("models/silero_vad.onnx");
    let whisper_path = root.join("models/ggml-tiny.en.bin");
    let jfk_path = root.join("benchmarks/local/jfk.wav");
    let silence_path = root.join("benchmarks/local/silence.wav");

    let vad = SileroVad::load(&vad_path, &VadModelProfile::silero_v5()).unwrap();
    let transcriber =
        NativeWhisperTranscriber::load(&whisper_path, &WhisperModelProfile::tiny_en()).unwrap();
    let segmenter = SpeechSegmenter::new(SegmenterConfig::default()).unwrap();
    let processor = TranscriptionProcessor::new(1, CaptionMode::EnEn, vad, segmenter, transcriber);

    let mut pipeline = Pipeline::default();
    let session = pipeline.start(processor).unwrap();

    // 1. Submit 3 seconds of real silence fixture: no captions should be produced.
    let silence = load_wav_16k_mono(&silence_path);
    for chunk in silence.chunks(MAX_BLOCK_SAMPLES) {
        pipeline.submit(session, chunk, false).unwrap();
    }
    std::thread::sleep(Duration::from_millis(150));
    assert!(pipeline.try_result().unwrap().is_none());

    // 2. Submit JFK speech fixture + trailing silence at real-time pace so no blocks are dropped.
    let mut speech = load_wav_16k_mono(&jfk_path);
    speech.extend(vec![0.0_f32; 20 * VAD_FRAME_SAMPLES]);

    let mut finals = Vec::new();
    for chunk in speech.chunks(MAX_BLOCK_SAMPLES) {
        pipeline.submit(session, chunk, false).unwrap();
        while let Some(msg) = pipeline.try_result().unwrap() {
            for update in msg.value {
                if update.is_final {
                    finals.push(update.text);
                }
            }
        }
        std::thread::sleep(Duration::from_secs_f64(chunk.len() as f64 / 16_000.0));
    }

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if let Some(msg) = pipeline.try_result().unwrap() {
            for update in msg.value {
                if update.is_final {
                    finals.push(update.text);
                }
            }
        }
        let joined = finals.join(" ").to_ascii_lowercase();
        if joined.contains("country") && joined.contains("do for you") {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    pipeline.stop().unwrap();

    let combined = finals.join(" ").to_ascii_lowercase();
    assert!(
        combined.contains("country") && combined.contains("do for you"),
        "expected JFK quote in final captions, got: {combined:?}"
    );
}
