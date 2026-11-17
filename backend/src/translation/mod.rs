//! Local text translation behind the [`Translator`] trait. Three engines
//! implement it today — `opus` (fast, CPU, English–Portuguese), `m2m`
//! (CPU, one model for every pair of 100 languages) and `riva` (GPU, 37
//! languages) — and captions never leave the machine. `ct2` is the native
//! wrapper the two CPU engines share.

mod ct2;
pub mod m2m;
pub mod opus;
pub mod riva;
pub mod stage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslatorError {
    LibraryUnavailable,
    ModelUnavailable,
    /// The model does not translate between the requested languages.
    UnsupportedLanguagePair,
    TranslationFailed,
}

/// A local text translator. Languages are BCP-47 codes as produced by the
/// recognizer (e.g. `en-US`, `pt-BR`); each implementation maps them to its
/// own model tags and rejects pairs it cannot serve.
#[cfg_attr(test, mockall::automock)]
pub trait Translator: Send {
    fn translate(
        &mut self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, TranslatorError>;
}
