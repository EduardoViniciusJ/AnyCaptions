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

/// Live translation: the interim text of an utterance is translated
/// provisionally as it grows, then the final replaces it. Prints when each
/// translation reached the sink relative to the interim that triggered it.
#[test]
#[ignore = "requires the opus_ct2_c build and the converted OPUS-MT models"]
fn provisional_translations_follow_the_interim_text() {
    use anycaptions_lib::{
        domain::{CaptionMode, PT_BR},
        ipc::CaptionUpdate,
        translation::stage::{StageEvent, TranslationStage},
    };
    use std::{
        sync::{Arc, Mutex},
        thread,
        time::Duration,
    };

    let translator =
        NativeOpusTranslator::load(&OpusPaths::resolve()).expect("opus library and models");
    let started = Instant::now();
    let emitted = Arc::new(Mutex::new(Vec::<(u128, CaptionUpdate)>::new()));
    let sink = Arc::clone(&emitted);
    let stage =
        TranslationStage::spawn(
            Box::new(translator),
            PT_BR,
            Some("en-US"),
            move |event| match event {
                StageEvent::Translated(update) => {
                    sink.lock()
                        .unwrap()
                        .push((started.elapsed().as_millis(), update));
                    true
                }
                StageEvent::Failed => false,
            },
        )
        .unwrap();
    let update = |revision: u32, text: &str, is_final: bool| CaptionUpdate {
        session_id: 1,
        segment_id: 1,
        revision,
        mode: CaptionMode::Translate(PT_BR),
        text: text.into(),
        is_final,
        language: Some("en-US".into()),
        speakers: Vec::new(),
        provisional: false,
    };
    // The recognizer's interim text, growing every 300 ms like live speech.
    let interims = [
        "Ask not",
        "Ask not what your",
        "Ask not what your country can",
        "Ask not what your country can do for",
        "Ask not what your country can do for you",
    ];
    let mut submitted = Vec::new();
    for (index, text) in interims.iter().enumerate() {
        submitted.push((started.elapsed().as_millis(), *text));
        stage
            .submit_partial(update(index as u32 + 1, text, false))
            .unwrap();
        thread::sleep(Duration::from_millis(300));
    }
    submitted.push((started.elapsed().as_millis(), "FINAL"));
    stage
        .submit(update(
            10,
            "Ask not what your country can do for you.",
            true,
        ))
        .unwrap();
    stage.finish();

    let emitted = emitted.lock().unwrap();
    for (at, text) in &submitted {
        eprintln!("[live] t={at:>5} ms  interim  {text}");
    }
    for (at, update) in emitted.iter() {
        eprintln!(
            "[live] t={at:>5} ms  {}  {}",
            if update.provisional {
                "provisional"
            } else {
                "FINAL      "
            },
            update.text
        );
    }
    let provisional: Vec<_> = emitted.iter().filter(|(_, u)| u.provisional).collect();
    assert!(
        provisional.len() >= 2,
        "expected several provisional translations"
    );
    assert!(
        provisional
            .iter()
            .all(|(_, u)| !u.text.trim().is_empty() && !u.is_final)
    );
    let last = &emitted.last().unwrap().1;
    assert!(last.is_final && !last.provisional);
    assert!(last.text.to_lowercase().contains("pa"), "{}", last.text);
}
