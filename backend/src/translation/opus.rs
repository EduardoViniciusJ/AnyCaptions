//! OPUS-MT (Helsinki-NLP) through the local `opus_ct2_c` wrapper: CTranslate2
//! int8 on the CPU plus SentencePiece, ~90 ms per caption sentence measured on
//! this project's hardware. Covers English–Portuguese both ways, one model per
//! direction; other pairs are rejected so the caller can pick another engine.

use std::path::{Path, PathBuf};

use super::{
    Translator, TranslatorError,
    ct2::{Ct2Library, Ct2Model, LIBRARY_FILENAME, translate_by_sentence},
    riva::model_tag,
};
use crate::models::{
    ModelProfile,
    catalog::{OPUS_MT_EN_PT, OPUS_MT_PT_EN},
};

/// Files the loader needs next to the verified `model.bin`.
const COMPANION_FILES: &[&str] = &[
    "config.json",
    "shared_vocabulary.json",
    SOURCE_SPM,
    TARGET_SPM,
];
const SOURCE_SPM: &str = "source.spm";
const TARGET_SPM: &str = "target.spm";

/// Where the wrapper library and the two model folders are found.
#[derive(Debug, Clone)]
pub struct OpusPaths {
    pub library_dir: PathBuf,
    pub en_pt: PathBuf,
    pub pt_en: PathBuf,
}

impl OpusPaths {
    pub fn resolve() -> Self {
        let root = crate::models::models_root();
        Self {
            library_dir: crate::native::nemo_library_dir(),
            en_pt: OPUS_MT_EN_PT.path_in(&root),
            pt_en: OPUS_MT_PT_EN.path_in(&root),
        }
    }

    pub fn translation_available(&self) -> bool {
        self.library_dir.join(LIBRARY_FILENAME).is_file()
            && direction_available(&OPUS_MT_EN_PT, &self.en_pt)
            && direction_available(&OPUS_MT_PT_EN, &self.pt_en)
    }
}

fn direction_available(profile: &ModelProfile, model_bin: &Path) -> bool {
    let Some(dir) = model_bin.parent() else {
        return false;
    };
    profile.verify(model_bin).is_ok() && COMPANION_FILES.iter().all(|name| dir.join(name).is_file())
}

/// A loaded OPUS translator holding both English–Portuguese directions.
/// One caption session owns it at a time, so `&mut self` keeps usage
/// single-threaded.
pub struct NativeOpusTranslator {
    en_pt: Ct2Model,
    pt_en: Ct2Model,
}

impl NativeOpusTranslator {
    pub fn load(paths: &OpusPaths) -> Result<Self, TranslatorError> {
        for (profile, model_bin) in [
            (&OPUS_MT_EN_PT, &paths.en_pt),
            (&OPUS_MT_PT_EN, &paths.pt_en),
        ] {
            if !direction_available(profile, model_bin) {
                return Err(TranslatorError::ModelUnavailable);
            }
        }
        let library = Ct2Library::load(&paths.library_dir)?;

        let load_start = std::time::Instant::now();
        let en_pt = load_direction(&library, &paths.en_pt)?;
        let pt_en = load_direction(&library, &paths.pt_en)?;
        // Timing only; translated content never reaches logs.
        eprintln!(
            "[opus] device=cpu load_ms={}",
            load_start.elapsed().as_millis()
        );
        let loaded = Self { en_pt, pt_en };
        // One tiny fixed warmup call per direction readies the thread pools, so
        // the first real caption pays no extra cost.
        let warmup = std::time::Instant::now();
        let _ = loaded.en_pt.translate("Ok.", ">>pob<<");
        let _ = loaded.pt_en.translate("Ok.", "");
        eprintln!("[opus] warmup_ms={}", warmup.elapsed().as_millis());
        Ok(loaded)
    }
}

fn load_direction(
    library: &std::sync::Arc<Ct2Library>,
    model_bin: &Path,
) -> Result<Ct2Model, TranslatorError> {
    let dir = model_bin
        .parent()
        .ok_or(TranslatorError::ModelUnavailable)?;
    Ct2Model::load(library, dir, &dir.join(SOURCE_SPM), &dir.join(TARGET_SPM))
}

impl Translator for NativeOpusTranslator {
    fn translate(
        &mut self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, TranslatorError> {
        if text.trim().is_empty() {
            return Ok(String::new());
        }
        let source = model_tag(source).ok_or(TranslatorError::UnsupportedLanguagePair)?;
        let target = model_tag(target).ok_or(TranslatorError::UnsupportedLanguagePair)?;
        if source == target {
            return Ok(text.to_owned());
        }
        // The multi-target en->pt model selects the variant by a leading token;
        // pt->en needs none. Every other pair belongs to another engine.
        let (model, prefix) = match (source.as_str(), target.as_str()) {
            ("en", "pt-br") => (&self.en_pt, ">>pob<<"),
            ("en", "pt-pt") => (&self.en_pt, ">>por<<"),
            ("pt-br" | "pt-pt", "en") => (&self.pt_en, ""),
            _ => return Err(TranslatorError::UnsupportedLanguagePair),
        };
        translate_by_sentence(text, |sentence| model.translate(sentence, prefix))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_library_or_model_is_reported_without_loading_anything() {
        let empty = std::env::temp_dir().join("anycaptions-missing-opus");
        let paths = OpusPaths {
            library_dir: empty.clone(),
            en_pt: empty.join("en-pt/model.bin"),
            pt_en: empty.join("pt-en/model.bin"),
        };
        assert!(matches!(
            NativeOpusTranslator::load(&paths),
            Err(TranslatorError::ModelUnavailable)
        ));
        assert!(!paths.translation_available());
    }
}
