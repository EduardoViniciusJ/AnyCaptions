use std::time::Instant;

use anycaptions_lib::translation::{Translator, TranslatorError, riva::NativeRivaTranslator};

/// Short caption-like sentences, the shape the translator will see in production.
const ENGLISH_SENTENCES: &[&str] = &[
    "Ask not what your country can do for you.",
    "The update will be released next week.",
    "I can't believe we won that match!",
    "Press the button to start the game.",
    "Thanks for watching, see you tomorrow.",
];

#[test]
#[ignore = "requires the NeMo-Speech.cpp NMT build and the Riva GGUF"]
fn riva_translates_english_captions_to_portuguese() {
    let paths = anycaptions_lib::translation::riva::RivaPaths::resolve();

    let load = Instant::now();
    let mut translator = NativeRivaTranslator::load(&paths).expect("NMT library and model");
    let load_ms = load.elapsed().as_millis();

    let mut total_ms = 0_u128;
    for sentence in ENGLISH_SENTENCES {
        let start = Instant::now();
        let translated = translator
            .translate(sentence, "en-US", "pt-BR")
            .expect("translation should succeed");
        let elapsed = start.elapsed().as_millis();
        total_ms += elapsed;
        eprintln!("[riva] {elapsed} ms | {sentence} -> {translated}");
        assert!(!translated.trim().is_empty());
        // The output must differ from the input; a copy means no translation ran.
        assert_ne!(translated.trim(), *sentence);
    }
    eprintln!(
        "[riva] load_ms={load_ms} avg_ms={}",
        total_ms / ENGLISH_SENTENCES.len() as u128
    );

    // Portuguese to Portuguese is a no-op, not a model call.
    let same = translator
        .translate("Olá, tudo bem?", "pt-BR", "pt-BR")
        .unwrap();
    assert_eq!(same, "Olá, tudo bem?");

    // Empty captions never reach the model.
    assert_eq!(translator.translate("   ", "en-US", "pt-BR").unwrap(), "");

    // A non-Portuguese target chosen in the UI reaches the model (variant tag).
    let spanish = translator
        .translate("Thanks for watching, see you tomorrow.", "en-US", "es-us")
        .expect("Spanish target should translate");
    eprintln!("[riva] en->es-us -> {spanish}");
    assert!(!spanish.trim().is_empty());
    assert_ne!(spanish.trim(), "Thanks for watching, see you tomorrow.");
}

#[test]
#[ignore = "requires both GGUFs; exercises ASR + NMT loaded together (VRAM check)"]
fn recognizer_and_translator_work_loaded_together() {
    use anycaptions_lib::transcription::{
        nemotron::{NativeNemotron, NemotronPaths},
        streaming::StreamingRecognizer,
    };

    let mut recognizer = NativeNemotron::load(
        &NemotronPaths::resolve(),
        None,
        anycaptions_lib::transcription::tuning::TRANSLATION,
    )
    .expect("ASR library and model");
    let mut translator =
        NativeRivaTranslator::load(&anycaptions_lib::translation::riva::RivaPaths::resolve())
            .expect("NMT library and model");

    let jfk = {
        let path =
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benchmarks/local/jfk.wav");
        let mut reader = hound::WavReader::open(path).expect("fixture wav should exist");
        reader
            .samples::<i16>()
            .map(|sample| f32::from(sample.unwrap()) / 32_768.0)
            .collect::<Vec<f32>>()
    };
    let mut finals = Vec::new();
    for block in jfk.chunks(2_048) {
        finals.extend(recognizer.push(block).unwrap());
    }
    finals.extend(recognizer.finish().unwrap());
    let english = finals
        .iter()
        .filter(|result| result.is_final)
        .map(|result| result.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(english.to_lowercase().contains("ask not"));

    let start = Instant::now();
    let portuguese = translator
        .translate(&english, "en-US", "pt-BR")
        .expect("translation with both models resident");
    eprintln!(
        "[riva+asr] translate_ms={} -> {portuguese}",
        start.elapsed().as_millis()
    );
    assert!(!portuguese.trim().is_empty());
    assert_ne!(portuguese.trim(), english.trim());
}

#[test]
#[ignore = "requires the NeMo-Speech.cpp NMT build and the Riva GGUF"]
fn riva_pivots_non_english_pairs_through_english() {
    let paths = anycaptions_lib::translation::riva::RivaPaths::resolve();
    let mut translator = NativeRivaTranslator::load(&paths).expect("NMT library and model");

    let start = Instant::now();
    let translated = translator
        .translate("¿Dónde está la biblioteca?", "es-ES", "pt-BR")
        .expect("pivot translation should succeed");
    eprintln!(
        "[riva] pivot es->pt {} ms -> {translated}",
        start.elapsed().as_millis()
    );
    assert!(!translated.trim().is_empty());

    // An unknown language is rejected before touching the model.
    assert_eq!(
        translator.translate("hello", "xx-XX", "pt-BR"),
        Err(TranslatorError::UnsupportedLanguagePair)
    );
}
