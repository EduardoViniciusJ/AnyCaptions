//! M2M100 418M (Meta, MIT) through the local `opus_ct2_c` wrapper: one
//! CTranslate2 int8 model on the CPU that translates directly between any two
//! of its 100 languages. Slower than OPUS-MT (about three times per sentence,
//! measured 2026-11-16) and a little less natural, but it needs no GPU and one
//! download covers every pair.

use std::path::PathBuf;

use super::{
    Translator, TranslatorError,
    ct2::{Ct2Library, Ct2Model, LIBRARY_FILENAME, translate_by_sentence},
};
use crate::models::{
    ModelProfile,
    catalog::{M2M100_418M, M2M100_CONFIG, M2M100_TOKENIZER, M2M100_VOCABULARY},
};

/// Every file the model folder must hold; all are verified against the catalog.
pub const M2M100_FILES: &[&ModelProfile] = &[
    &M2M100_CONFIG,
    &M2M100_VOCABULARY,
    &M2M100_TOKENIZER,
    &M2M100_418M,
];

/// The model's language codes: the `__xx__` tokens of its vocabulary.
const LANGUAGES: &[&str] = &[
    "af", "am", "ar", "ast", "az", "ba", "be", "bg", "bn", "br", "bs", "ca", "ceb", "cs", "cy",
    "da", "de", "el", "en", "es", "et", "fa", "ff", "fi", "fr", "fy", "ga", "gd", "gl", "gu", "ha",
    "he", "hi", "hr", "ht", "hu", "hy", "id", "ig", "ilo", "is", "it", "ja", "jv", "ka", "kk",
    "km", "kn", "ko", "lb", "lg", "ln", "lo", "lt", "lv", "mg", "mk", "ml", "mn", "mr", "ms", "my",
    "ne", "nl", "no", "ns", "oc", "or", "pa", "pl", "ps", "pt", "ro", "ru", "sd", "si", "sk", "sl",
    "so", "sq", "sr", "ss", "su", "sv", "sw", "ta", "th", "tl", "tn", "tr", "uk", "ur", "uz", "vi",
    "wo", "xh", "yi", "yo", "zh", "zu",
];

/// Maps a language code (BCP-47, e.g. `en-US`, `pt-br`, `zh-CN`) to the
/// model's code (`en`, `pt`, `zh`). The model has one code per language, so
/// regional variants share it; Norwegian Bokmål and Nynorsk use `no`.
pub(crate) fn language_code(tag: &str) -> Option<&'static str> {
    let lower = tag.replace('_', "-").to_ascii_lowercase();
    let primary = match lower.split('-').next()? {
        "nb" | "nn" => "no",
        "fil" => "tl",
        other => other,
    };
    LANGUAGES.iter().copied().find(|code| *code == primary)
}

fn language_token(code: &str) -> String {
    format!("__{code}__")
}

/// Where the wrapper library and the model folder are found.
#[derive(Debug, Clone)]
pub struct M2mPaths {
    pub library_dir: PathBuf,
    /// Folder holding every file of [`M2M100_FILES`].
    pub model_dir: PathBuf,
}

impl M2mPaths {
    pub fn resolve() -> Self {
        let root = crate::models::models_root();
        Self {
            library_dir: crate::native::nemo_library_dir(),
            model_dir: M2M100_418M
                .path_in(&root)
                .parent()
                .map(PathBuf::from)
                .unwrap_or_default(),
        }
    }

    /// The library is present and every model file is installed and intact.
    pub fn translation_available(&self) -> bool {
        self.library_dir.join(LIBRARY_FILENAME).is_file() && self.files_verified()
    }

    fn files_verified(&self) -> bool {
        M2M100_FILES.iter().all(|profile| {
            profile
                .verify(&self.model_dir.join(profile.filename))
                .is_ok()
        })
    }
}

/// The loaded model. One caption session owns it at a time, so `&mut self`
/// keeps usage single-threaded.
pub struct NativeM2mTranslator {
    model: Ct2Model,
}

impl NativeM2mTranslator {
    pub fn load(paths: &M2mPaths) -> Result<Self, TranslatorError> {
        if !paths.files_verified() {
            return Err(TranslatorError::ModelUnavailable);
        }
        let library = Ct2Library::load(&paths.library_dir)?;
        if !library.supports_target_tokens() {
            // A wrapper older than 0.2.0: it only knows the OPUS call.
            return Err(TranslatorError::LibraryUnavailable);
        }
        let load_start = std::time::Instant::now();
        // Source and target share one SentencePiece model.
        let tokenizer = paths.model_dir.join(M2M100_TOKENIZER.filename);
        let model = Ct2Model::load(&library, &paths.model_dir, &tokenizer, &tokenizer)?;
        // Timing only; translated content never reaches logs.
        eprintln!(
            "[m2m100] device=cpu load_ms={}",
            load_start.elapsed().as_millis()
        );
        let warmup = std::time::Instant::now();
        let _ = model.translate_to("Ok.", &language_token("en"), &language_token("pt"));
        eprintln!("[m2m100] warmup_ms={}", warmup.elapsed().as_millis());
        Ok(Self { model })
    }
}

impl Translator for NativeM2mTranslator {
    fn translate(
        &mut self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, TranslatorError> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }
        let source = language_code(source).ok_or(TranslatorError::UnsupportedLanguagePair)?;
        let target = language_code(target).ok_or(TranslatorError::UnsupportedLanguagePair)?;
        if source == target {
            return Ok(text.to_owned());
        }
        let (source, target) = (language_token(source), language_token(target));
        translate_by_sentence(text, |sentence| {
            self.model.translate_to(sentence, &source, &target)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ASR_LANGUAGES, TRANSLATION_TARGETS, TranslationEngine};
    use crate::translation::stage::resolve_text_source;

    #[test]
    fn language_tags_map_to_the_model_codes() {
        assert_eq!(language_code("en-US"), Some("en"));
        assert_eq!(language_code("pt-br"), Some("pt"));
        assert_eq!(language_code("pt_PT"), Some("pt"));
        assert_eq!(language_code("zh-CN"), Some("zh"));
        assert_eq!(language_code("nb"), Some("no"));
        assert_eq!(language_code("ja-JP"), Some("ja"));
        assert_eq!(language_code("xx"), None);
        assert_eq!(language_code(""), None);
        assert_eq!(language_token("pt"), "__pt__");
    }

    #[test]
    fn every_offered_target_is_a_model_language() {
        for target in TranslationEngine::M2m100.targets() {
            assert!(
                language_code(target.tag()).is_some(),
                "{} is offered but the model cannot write it",
                target.tag()
            );
        }
    }

    #[test]
    fn dictation_can_use_every_target_it_may_have_saved() {
        // Dictation keeps its own target, chosen under any engine. The ones
        // this engine does not list (Portuguese of Portugal, Latin American
        // Spanish, Traditional Chinese) still translate, to the shared code.
        for target in TRANSLATION_TARGETS {
            assert!(language_code(target.tag()).is_some(), "{}", target.tag());
        }
        assert_eq!(language_code("pt-pt"), Some("pt"));
        assert_eq!(language_code("es-us"), Some("es"));
        assert_eq!(language_code("zh-tw"), Some("zh"));
    }

    #[test]
    fn dictation_without_a_fixed_language_detects_one_the_model_reads() {
        // (dictated text, the model code its detected language maps to)
        let dictated = [
            (
                "Amanhã de manhã eu vou enviar o relatório para a equipe.",
                "pt",
            ),
            (
                "Tomorrow morning I will send the report to the whole team.",
                "en",
            ),
            ("Завтра утром я отправлю отчёт всей команде.", "ru"),
            ("明日の朝、チーム全員にレポートを送ります。", "ja"),
            (
                "Demain matin, j'enverrai le rapport à toute l'équipe.",
                "fr",
            ),
            (
                "Morgen früh schicke ich den Bericht an das ganze Team.",
                "de",
            ),
        ];
        for (text, code) in dictated {
            assert_eq!(
                language_code(resolve_text_source(text)),
                Some(code),
                "{text}"
            );
        }
        // Too little text to detect falls back to English, which the model has.
        assert!(language_code(resolve_text_source("")).is_some());
    }

    #[test]
    fn the_main_spoken_languages_are_understood() {
        // Not every language the recognizer hears exists in the model (it
        // has 100); the ones most people will meet must.
        for tag in [
            "en-US", "es-ES", "zh-CN", "hi", "ar", "fr", "de", "ja-JP", "ru", "pt-BR", "ko", "it",
            "nl", "pl", "tr", "uk",
        ] {
            assert!(ASR_LANGUAGES.contains(&tag));
            assert!(language_code(tag).is_some(), "{tag}");
        }
    }

    #[test]
    fn missing_library_or_model_is_reported_without_loading_anything() {
        let empty = std::env::temp_dir().join("anycaptions-missing-m2m");
        let paths = M2mPaths {
            library_dir: empty.clone(),
            model_dir: empty.join("m2m100-418m"),
        };
        assert!(matches!(
            NativeM2mTranslator::load(&paths),
            Err(TranslatorError::ModelUnavailable)
        ));
        assert!(!paths.translation_available());
    }
}
