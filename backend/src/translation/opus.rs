//! OPUS-MT (Helsinki-NLP) through the local `opus_ct2_c` wrapper: CTranslate2
//! int8 on the CPU plus SentencePiece, ~90 ms per caption sentence measured on
//! this project's hardware. Covers English–Portuguese both ways; other pairs
//! are rejected so the caller can fall back to the Riva engine. The library is
//! loaded at runtime, so builds and unit tests never need it.

use std::{
    ffi::{CStr, CString, c_char},
    path::PathBuf,
    ptr,
};

use libloading::Library;

use super::{Translator, TranslatorError, riva::model_tag};
use crate::{
    models::catalog::{OPUS_MT_EN_PT, OPUS_MT_PT_EN},
    native::open_library,
};

#[cfg(windows)]
pub const LIBRARY_FILENAME: &str = "opus_ct2_c.dll";
#[cfg(not(windows))]
pub const LIBRARY_FILENAME: &str = "libopus_ct2_c.so";

/// Files the CTranslate2 loader needs next to the verified `model.bin`.
const COMPANION_FILES: &[&str] = &["config.json", "shared_vocabulary.json"];

const STATUS_OK: i32 = 0;
const STATUS_INVALID_ARGUMENT: i32 = 1;

/// Measured on the 16-thread reference CPU: 4 threads averaged 149 ms per
/// sentence, 8 reached 100 ms and 12 gained nothing more.
const INTRA_THREADS: i32 = 8;

// ---- C ABI mirror of opus_ct2_c.h. ----

#[repr(C)]
struct NativeTranslator {
    _opaque: [u8; 0],
}

type CreateFn = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    *const c_char,
    i32,
    *mut *mut NativeTranslator,
) -> i32;
type DestroyFn = unsafe extern "C" fn(*mut NativeTranslator);
type TranslateFn = unsafe extern "C" fn(
    *mut NativeTranslator,
    *const c_char,
    *const c_char,
    *mut *mut c_char,
) -> i32;
type ResultDestroyFn = unsafe extern "C" fn(*mut c_char);

/// Function pointers resolved from the library; valid while `_library` is loaded.
struct Api {
    create: CreateFn,
    destroy: DestroyFn,
    translate: TranslateFn,
    result_destroy: ResultDestroyFn,
    // Declared last so it is dropped after every handle that uses these pointers.
    _library: Library,
}

impl Api {
    fn load(library_dir: &std::path::Path) -> Result<Self, TranslatorError> {
        let path = library_dir.join(LIBRARY_FILENAME);
        let library = open_library(&path).map_err(|_| TranslatorError::LibraryUnavailable)?;
        // SAFETY: the names and signatures below follow opus_ct2_c.h of the local
        // wrapper build; each pointer is copied out while `library` stays alive.
        unsafe {
            macro_rules! symbol {
                ($name:literal) => {
                    *library
                        .get($name)
                        .map_err(|_| TranslatorError::LibraryUnavailable)?
                };
            }
            Ok(Self {
                create: symbol!(b"opus_ct2_create\0"),
                destroy: symbol!(b"opus_ct2_destroy\0"),
                translate: symbol!(b"opus_ct2_translate\0"),
                result_destroy: symbol!(b"opus_ct2_result_destroy\0"),
                _library: library,
            })
        }
    }
}

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

fn direction_available(profile: &crate::models::ModelProfile, model_bin: &std::path::Path) -> bool {
    let Some(dir) = model_bin.parent() else {
        return false;
    };
    profile.verify(model_bin).is_ok()
        && COMPANION_FILES
            .iter()
            .chain(["source.spm", "target.spm"].iter())
            .all(|name| dir.join(name).is_file())
}

/// One loaded direction (source SentencePiece + CTranslate2 model + target
/// SentencePiece).
struct Direction {
    translator: *mut NativeTranslator,
}

/// A loaded OPUS translator holding both English–Portuguese directions.
/// One caption session owns it at a time, so `&mut self` keeps usage
/// single-threaded.
pub struct NativeOpusTranslator {
    en_pt: Direction,
    pt_en: Direction,
    api: Api,
}

// SAFETY: the handles are owned exclusively by this value; CTranslate2 allows
// calls from any thread.
unsafe impl Send for NativeOpusTranslator {}

impl NativeOpusTranslator {
    pub fn load(paths: &OpusPaths) -> Result<Self, TranslatorError> {
        for (profile, model_bin) in [
            (&OPUS_MT_EN_PT, &paths.en_pt),
            (&OPUS_MT_PT_EN, &paths.pt_en),
        ] {
            profile
                .verify(model_bin)
                .map_err(|_| TranslatorError::ModelUnavailable)?;
            if !direction_available(profile, model_bin) {
                return Err(TranslatorError::ModelUnavailable);
            }
        }
        let api = Api::load(&paths.library_dir)?;

        let load_start = std::time::Instant::now();
        let en_pt = create_direction(&api, &paths.en_pt)?;
        let pt_en = match create_direction(&api, &paths.pt_en) {
            Ok(direction) => direction,
            Err(error) => {
                // SAFETY: `en_pt` was created by this library and is released once.
                unsafe { (api.destroy)(en_pt.translator) };
                return Err(error);
            }
        };
        // Timing only; translated content never reaches logs.
        eprintln!(
            "[opus] device=cpu load_ms={}",
            load_start.elapsed().as_millis()
        );
        let mut loaded = Self { en_pt, pt_en, api };
        // One tiny fixed warmup call per direction readies the thread pools, so
        // the first real caption pays no extra cost.
        let warmup = std::time::Instant::now();
        let _ = loaded.translate_direction(true, "Ok.", ">>pob<<");
        let _ = loaded.translate_direction(false, "Ok.", "");
        eprintln!("[opus] warmup_ms={}", warmup.elapsed().as_millis());
        Ok(loaded)
    }

    fn translate_direction(
        &mut self,
        english_to_portuguese: bool,
        text: &str,
        prefix: &str,
    ) -> Result<String, TranslatorError> {
        let direction = if english_to_portuguese {
            &self.en_pt
        } else {
            &self.pt_en
        };
        let text = CString::new(text).map_err(|_| TranslatorError::TranslationFailed)?;
        let prefix = CString::new(prefix).map_err(|_| TranslatorError::TranslationFailed)?;
        let mut result: *mut c_char = ptr::null_mut();
        // SAFETY: the pointers are valid for the call; a non-null result is owned
        // by us and destroyed below.
        let status = unsafe {
            (self.api.translate)(
                direction.translator,
                text.as_ptr(),
                prefix.as_ptr(),
                &mut result,
            )
        };
        if status == STATUS_INVALID_ARGUMENT {
            return Err(TranslatorError::UnsupportedLanguagePair);
        }
        if status != STATUS_OK || result.is_null() {
            return Err(TranslatorError::TranslationFailed);
        }
        // SAFETY: `result` is a NUL-terminated string owned by us until destroyed.
        unsafe {
            let translated = CStr::from_ptr(result).to_string_lossy().into_owned();
            (self.api.result_destroy)(result);
            Ok(translated)
        }
    }
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
        // pt->en needs none. Every other pair belongs to the fallback engine.
        match (source.as_str(), target.as_str()) {
            ("en", "pt-br") => self.translate_direction(true, text, ">>pob<<"),
            ("en", "pt-pt") => self.translate_direction(true, text, ">>por<<"),
            ("pt-br" | "pt-pt", "en") => self.translate_direction(false, text, ""),
            _ => Err(TranslatorError::UnsupportedLanguagePair),
        }
    }
}

impl Drop for NativeOpusTranslator {
    fn drop(&mut self) {
        // SAFETY: both handles were created by this library and are released
        // once; `api` (and the library) is dropped only after this.
        unsafe {
            (self.api.destroy)(self.en_pt.translator);
            (self.api.destroy)(self.pt_en.translator);
        }
    }
}

fn create_direction(api: &Api, model_bin: &std::path::Path) -> Result<Direction, TranslatorError> {
    let dir = model_bin
        .parent()
        .ok_or(TranslatorError::ModelUnavailable)?;
    let as_cstring = |path: PathBuf| {
        path.to_str()
            .and_then(|value| CString::new(value).ok())
            .ok_or(TranslatorError::ModelUnavailable)
    };
    let model_dir = as_cstring(dir.to_path_buf())?;
    let source_spm = as_cstring(dir.join("source.spm"))?;
    let target_spm = as_cstring(dir.join("target.spm"))?;
    let mut translator = ptr::null_mut();
    // SAFETY: every pointer refers to a live CString; the handle is checked below.
    let status = unsafe {
        (api.create)(
            model_dir.as_ptr(),
            source_spm.as_ptr(),
            target_spm.as_ptr(),
            INTRA_THREADS,
            &mut translator,
        )
    };
    if status != STATUS_OK || translator.is_null() {
        return Err(TranslatorError::ModelUnavailable);
    }
    Ok(Direction { translator })
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
