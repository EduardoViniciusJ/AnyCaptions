use std::{path::PathBuf, time::Instant};

use anycaptions_lib::transcription::{
    nemotron::{NativeNemotron, NemotronPaths},
    streaming::{SpeechResult, StreamingRecognizer},
};

// Same block size the capture coordinator submits (128 ms at 16 kHz).
const BLOCK_SAMPLES: usize = 2_048;

fn load_wav_16k_mono(path: &PathBuf) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).expect("fixture wav should exist");
    let spec = reader.spec();
    assert_eq!((spec.channels, spec.sample_rate), (1, 16_000));
    reader
        .samples::<i16>()
        .map(|sample| f32::from(sample.unwrap()) / 32_768.0)
        .collect()
}

fn stream(recognizer: &mut NativeNemotron, samples: &[f32]) -> Vec<SpeechResult> {
    let mut results = Vec::new();
    for block in samples.chunks(BLOCK_SAMPLES) {
        results.extend(recognizer.push(block).unwrap());
    }
    results.extend(recognizer.finish().unwrap());
    results
}

#[test]
#[ignore = "requires the NeMo-Speech.cpp build, the Nemotron GGUF and jfk.wav"]
fn nemotron_streams_real_english_speech_and_ignores_silence() {
    let paths = NemotronPaths::resolve();
    let jfk = load_wav_16k_mono(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benchmarks/local/jfk.wav"),
    );

    let load = Instant::now();
    let mut recognizer = NativeNemotron::load(&paths).expect("NeMo library and model");
    let load_ms = load.elapsed().as_millis();

    let decode = Instant::now();
    let results = stream(&mut recognizer, &jfk);
    let decode_ms = decode.elapsed().as_millis();
    eprintln!(
        "[nemotron] load_ms={load_ms} decode_ms={decode_ms} audio_ms={}",
        jfk.len() / 16
    );

    let finals: Vec<_> = results.iter().filter(|result| result.is_final).collect();
    let text = finals
        .iter()
        .map(|result| result.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    assert!(
        text.contains("ask not what your country can do for you"),
        "{text}"
    );
    assert!(
        results.iter().any(|result| !result.is_final),
        "interim results are expected while speaking"
    );

    let mut quiet = NativeNemotron::load(&paths).expect("NeMo library and model");
    let silence = stream(&mut quiet, &vec![0.0; 16_000 * 3]);
    assert!(silence.iter().all(|result| result.text.trim().is_empty()));
}
