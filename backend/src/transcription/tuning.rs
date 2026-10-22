//! Per-mode recognizer tuning. The plain transcription modes keep the library
//! defaults untouched; the translate mode closes utterances sooner so the
//! translation of each sentence starts earlier.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecognizerTuning {
    /// Silence that ends an utterance, in milliseconds; `<= 0` keeps the
    /// library default (800 ms in the pinned NeMo-Speech.cpp build).
    pub stop_history_eou_ms: i32,
}

/// Transcription modes (Auto): library defaults, deliberately untouched.
pub const TRANSCRIPTION: RecognizerTuning = RecognizerTuning {
    stop_history_eou_ms: 0,
};

/// Translate mode: finals close after 150 ms of silence instead of 800 ms, so
/// translation starts ~0.65 s sooner (the user chose minimum latency knowing
/// sentences may split at micro-pauses).
pub const TRANSLATION: RecognizerTuning = RecognizerTuning {
    stop_history_eou_ms: 150,
};
