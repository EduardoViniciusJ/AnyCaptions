use std::{path::PathBuf, time::Instant};

use anycaptions_lib::transcription::{
    nemotron::{NativeNemotron, NemotronModel, NemotronPaths},
    streaming::{SpeechResult, StreamingRecognizer},
    tuning::{TRANSCRIPTION, TRANSLATION},
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

fn jfk_samples() -> Vec<f32> {
    load_wav_16k_mono(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benchmarks/local/jfk.wav"))
}

fn stream(recognizer: &mut NativeNemotron, samples: &[f32]) -> Vec<SpeechResult> {
    let mut results = Vec::new();
    for block in samples.chunks(BLOCK_SAMPLES) {
        results.extend(recognizer.push(block).unwrap());
    }
    results.extend(recognizer.finish().unwrap());
    results
}

fn final_text(results: &[SpeechResult]) -> String {
    results
        .iter()
        .filter(|result| result.is_final)
        .map(|result| result.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[test]
#[ignore = "requires the NeMo-Speech.cpp build, the Nemotron GGUF and jfk.wav"]
fn nemotron_streams_real_english_speech_and_ignores_silence() {
    let paths = NemotronPaths::resolve();
    let jfk = jfk_samples();

    let load = Instant::now();
    let mut recognizer =
        NativeNemotron::load(&paths, None, TRANSCRIPTION).expect("NeMo library and model");
    let load_ms = load.elapsed().as_millis();

    let decode = Instant::now();
    let results = stream(&mut recognizer, &jfk);
    let decode_ms = decode.elapsed().as_millis();
    eprintln!(
        "[nemotron] load_ms={load_ms} decode_ms={decode_ms} audio_ms={}",
        jfk.len() / 16
    );

    let text = final_text(&results);
    assert!(
        text.contains("ask not what your country can do for you"),
        "{text}"
    );
    assert!(
        results.iter().any(|result| !result.is_final),
        "interim results are expected while speaking"
    );
    // Not every final carries a language code (observed: only some do), but when
    // present it must match the spoken language.
    let detected: Vec<_> = results
        .iter()
        .filter(|result| result.is_final && !result.text.trim().is_empty())
        .map(|result| result.language.as_deref().unwrap_or(""))
        .collect();
    assert!(
        detected.iter().any(|code| code.starts_with("en")),
        "at least one final should carry the detected language: {detected:?}"
    );
    assert!(
        detected
            .iter()
            .all(|code| code.is_empty() || code.starts_with("en")),
        "no final should carry a wrong language: {detected:?}"
    );

    let mut quiet =
        NativeNemotron::load(&paths, None, TRANSCRIPTION).expect("NeMo library and model");
    let silence = stream(&mut quiet, &vec![0.0; 16_000 * 3]);
    assert!(silence.iter().all(|result| result.text.trim().is_empty()));
}

#[test]
#[ignore = "requires the NeMo-Speech.cpp build, the Nemotron GGUF and jfk.wav"]
fn one_loaded_model_serves_streams_with_different_endpointing() {
    let paths = NemotronPaths::resolve();
    let jfk = jfk_samples();

    let load = Instant::now();
    let model = NemotronModel::load(&paths, TRANSCRIPTION.rnnt_right_context)
        .expect("NeMo library and model");
    let load_ms = load.elapsed().as_millis();

    // Two sequential streams over the same default model.
    let mut auto = NativeNemotron::open(model.clone(), None, TRANSCRIPTION).expect("stream");
    let auto_results = stream(&mut auto, &jfk);
    drop(auto);
    // The translate mode loads its own model (bigger right context chosen at
    // load time), exactly like the factory does.
    let translate_model = NemotronModel::load(&paths, TRANSLATION.rnnt_right_context)
        .expect("NeMo library and model");
    let mut translate = NativeNemotron::open(translate_model, None, TRANSLATION).expect("stream");
    let translate_results = stream(&mut translate, &jfk);

    let auto_finals = auto_results.iter().filter(|r| r.is_final).count();
    let translate_finals = translate_results.iter().filter(|r| r.is_final).count();
    eprintln!(
        "[nemotron] shared model: load_ms={load_ms} finals_default={auto_finals} finals_150ms={translate_finals}"
    );

    for results in [&auto_results, &translate_results] {
        let text = final_text(results);
        assert!(
            text.contains("ask not what your country can do for you"),
            "{text}"
        );
    }
    // The per-stream 150 ms endpointing must actually apply: shorter silences
    // close utterances, so the same audio yields more finals than the default.
    assert!(
        translate_finals > auto_finals,
        "expected the 150 ms stream to split more: default={auto_finals} 150ms={translate_finals}"
    );
}

#[test]
#[ignore = "requires the NeMo-Speech.cpp build, the Nemotron GGUF and jfk.wav"]
fn forcing_a_language_keeps_matches_and_filters_other_speech() {
    let paths = NemotronPaths::resolve();
    let jfk = jfk_samples();

    // English speech with English forced is kept.
    let mut english =
        NativeNemotron::load(&paths, Some("en-US"), TRANSCRIPTION).expect("NeMo library and model");
    let kept = final_text(&stream(&mut english, &jfk));
    assert!(
        kept.contains("ask not what your country can do for you"),
        "{kept}"
    );

    // English speech with Portuguese forced is filtered out, not mistranscribed.
    let mut portuguese =
        NativeNemotron::load(&paths, Some("pt-BR"), TRANSCRIPTION).expect("NeMo library and model");
    let filtered = final_text(&stream(&mut portuguese, &jfk));
    assert!(filtered.trim().is_empty(), "{filtered}");
}
