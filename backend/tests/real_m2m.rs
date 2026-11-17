//! Real-hardware checks for the M2M100 translator (CTranslate2 int8, CPU).
//! Run explicitly: cargo test --test real_m2m -- --ignored --nocapture

use std::time::Instant;

use anycaptions_lib::translation::{
    Translator, TranslatorError,
    m2m::{M2mPaths, NativeM2mTranslator},
};

#[test]
#[ignore = "requires opus_ct2_c 0.2.0 and the downloaded M2M100 model"]
fn m2m100_translates_directly_between_languages() {
    let paths = M2mPaths::resolve();
    assert!(paths.translation_available(), "library and model files");
    let load = Instant::now();
    let mut translator = NativeM2mTranslator::load(&paths).expect("m2m100 library and model");
    let load_ms = load.elapsed().as_millis();

    // (source, target, text, a word the translation must contain)
    let cases = [
        (
            "en-US",
            "pt-br",
            "The update will be released next week.",
            "semana",
        ),
        (
            "pt-BR",
            "en",
            "Obrigado por assistir, até amanhã.",
            "tomorrow",
        ),
        (
            "ru",
            "fr",
            "Я думаю, что мы должны начать встречу прямо сейчас.",
            "nous",
        ),
        (
            "ja-JP",
            "pt-br",
            "明日の朝、新しいバージョンをリリースする予定です。",
            "vers",
        ),
        (
            "de",
            "es-es",
            "Ich glaube nicht, dass das eine gute Idee ist.",
            "idea",
        ),
    ];
    let mut total_ms = 0_u128;
    for (source, target, sentence, expected) in cases {
        let start = Instant::now();
        let text = translator
            .translate(sentence, source, target)
            .expect("supported pair");
        let elapsed = start.elapsed().as_millis();
        total_ms += elapsed;
        eprintln!("[m2m100] {elapsed} ms | {source}->{target} | {text}");
        assert!(
            text.to_lowercase().contains(expected),
            "{source}->{target}: {text}"
        );
    }
    eprintln!(
        "[m2m100] load_ms={load_ms} avg_ms={}",
        total_ms / cases.len() as u128
    );

    // Merged captions keep every sentence.
    let merged = translator
        .translate(
            "Can you hear me? I think my microphone is broken.",
            "en",
            "pt-br",
        )
        .expect("en->pt");
    eprintln!("[m2m100] merged -> {merged}");
    assert!(merged.contains('?'), "the question survives: {merged}");

    // Dictation pastes whole paragraphs: every sentence must come through,
    // in order, and into the regional targets only dictation can still hold.
    let dictated = "Bom dia, pessoal. Eu terminei o relatório ontem à noite. \
                    Vocês podem revisar até sexta-feira?";
    let pasted = translator
        .translate(dictated, "pt-BR", "en")
        .expect("pt->en");
    eprintln!("[m2m100] dictation -> {pasted}");
    let lower = pasted.to_lowercase();
    let report = lower.find("report").expect("second sentence");
    let friday = lower.find("friday").expect("third sentence");
    assert!(report < friday, "sentences keep their order: {pasted}");
    assert!(pasted.trim_end().ends_with('?'), "{pasted}");
    let regional = translator
        .translate("The meeting starts at nine.", "en-US", "es-us")
        .expect("en->es-us");
    assert!(regional.to_lowercase().contains("reuni"), "{regional}");

    // Same language passes through; a language outside the model is rejected.
    assert_eq!(
        translator.translate("Hello.", "en-US", "en").as_deref(),
        Ok("Hello.")
    );
    assert_eq!(
        translator.translate("Hello.", "en", "haw-US"),
        Err(TranslatorError::UnsupportedLanguagePair)
    );
}
