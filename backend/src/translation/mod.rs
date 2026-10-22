//! Local text translation behind the [`Translator`] trait. The engine in use
//! (`riva`) is one implementation; captions never leave the machine.

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
