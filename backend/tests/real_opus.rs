//! Real-hardware checks for the OPUS-MT translator (CTranslate2 int8, CPU).
//! Run explicitly: cargo test --test real_opus -- --ignored --nocapture

use std::time::Instant;

use anycaptions_lib::translation::{
    Translator, TranslatorError,
    opus::{NativeOpusTranslator, OpusPaths},
};

#[test]
#[ignore = "requires the opus_ct2_c build and the converted OPUS-MT models"]
fn opus_translates_both_directions_quickly() {
    let paths = OpusPaths::resolve();
    let load = Instant::now();
    let mut translator = NativeOpusTranslator::load(&paths).expect("opus library and models");
    let load_ms = load.elapsed().as_millis();

    let english = [
        "Ask not what your country can do for you.",
        "The update will be released next week.",
        "I can't believe we won that match!",
        "Press the button to start the game.",
        "Thanks for watching, see you tomorrow.",
    ];
    let mut total_ms = 0_u128;
    for sentence in english {
        let start = Instant::now();
        let text = translator
            .translate(sentence, "en-US", "pt-br")
            .expect("en->pt");
        let elapsed = start.elapsed().as_millis();
        total_ms += elapsed;
        eprintln!("[opus] {elapsed} ms | {sentence} -> {text}");
        assert!(!text.trim().is_empty());
    }
    eprintln!(
        "[opus] load_ms={load_ms} avg_ms={}",
        total_ms / english.len() as u128
    );

    let back = translator
        .translate("Obrigado por assistir, até amanhã.", "pt-BR", "en")
        .expect("pt->en");
    eprintln!("[opus] pt->en -> {back}");
    assert!(back.to_lowercase().contains("tomorrow"));

    // European Portuguese target selects the >>por<< variant.
    let european = translator
        .translate("The update will be released next week.", "en", "pt-pt")
        .expect("en->pt-pt");
    eprintln!("[opus] en->pt-pt -> {european}");
    assert!(!european.trim().is_empty());

    // Same language passes through; unsupported pairs are rejected for the
    // Riva fallback to take over.
    assert_eq!(
        translator.translate("Hello.", "en", "en").as_deref(),
        Ok("Hello.")
    );
    assert!(matches!(
        translator.translate("Bonjour.", "fr", "de"),
        Err(TranslatorError::UnsupportedLanguagePair)
    ));
}
