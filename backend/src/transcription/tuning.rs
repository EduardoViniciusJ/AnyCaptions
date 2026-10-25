//! Per-mode recognizer tuning. The plain transcription modes keep the library
//! defaults untouched; the translate mode trades a little latency for fuller,
//! more accurate sentences, which also translate better.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecognizerTuning {
    /// Silence that ends an utterance, in milliseconds; `<= 0` keeps the
    /// library default (800 ms in the pinned NeMo-Speech.cpp build).
    /// Applied per stream.
    pub stop_history_eou_ms: i32,
    /// Cache-aware RNNT right context, in encoder frames (160 ms each):
    /// how much "future" the model hears before deciding each word.
    /// `None` keeps the library default (1, the low-latency preset);
    /// `Some(-1)` selects the model maximum (most accurate, more delay).
    /// Applied when the model loads, so modes with different values use
    /// separate recognizer instances.
    pub rnnt_right_context: Option<i32>,
}

/// Transcription modes (Auto) and dictation: library defaults, deliberately
/// untouched. The maximum right context (`Some(-1)` → right=3, 320 ms steps)
/// was tried on 2026-10-25 and reverted: the user preferred the speed of the
/// low-latency preset (right=1, 160 ms steps).
pub const TRANSCRIPTION: RecognizerTuning = RecognizerTuning {
    stop_history_eou_ms: 0,
    rnnt_right_context: None,
};

/// Translate mode: silence that closes a final, in milliseconds. 150 is the
/// minimum-latency choice (sentences split at micro-pauses) and 250 keeps
/// sentences whole; 200 is the middle ground, with the stage merging any
/// leftover tiny sentences before translating.
pub const TRANSLATION_EOU_MS: i32 = 200;

/// Translate mode: right context preset. `None` keeps the low-latency preset
/// the Auto mode uses (the model maximum, `Some(-1)`, recognized better but
/// the extra delay was rejected in user testing).
pub const TRANSLATION_RIGHT_CONTEXT: Option<i32> = None;

/// Translate mode: whole sentences on the fast preset; only the endpointing
/// differs from Auto, so both modes share one loaded model.
pub const TRANSLATION: RecognizerTuning = RecognizerTuning {
    stop_history_eou_ms: TRANSLATION_EOU_MS,
    rnnt_right_context: TRANSLATION_RIGHT_CONTEXT,
};
