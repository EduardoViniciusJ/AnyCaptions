//! Riva-Translate through the NeMo-Speech.cpp NMT C API (`include/nemo_speech/nmt.h`,
//! v0.2.0, llama.cpp backend). The library is loaded at runtime, so builds and unit
//! tests never need it.

use std::{
    ffi::{CStr, CString, c_char},
    path::PathBuf,
    ptr,
};

use libloading::Library;

use super::{Translator, TranslatorError};
use crate::{models::catalog::RIVA_TRANSLATE_Q8, native::open_library};

#[cfg(windows)]
pub const LIBRARY_FILENAME: &str = "nemo_speech_nmt_c.dll";
#[cfg(not(windows))]
pub const LIBRARY_FILENAME: &str = "libnemo_speech_nmt_c.so";

/// Every supported pair includes English, so other pairs hop through it.
const ENGLISH: &str = "en";

/// Maps a recognizer language (BCP-47, e.g. `en-US`) to the model's tag (e.g. `en`).
/// Portuguese targets the Brazilian variant the model translates into.
fn model_tag(code: &str) -> Option<String> {
    let primary = code.split(['-', '_']).next()?.to_ascii_lowercase();
    if primary.len() != 2 || !primary.bytes().all(|b| b.is_ascii_lowercase()) {
        return None;
    }
    Some(if primary == "pt" {
        "pt-br".to_owned()
    } else {
        primary
    })
}

// ---- C ABI mirror of nmt.h. Field order and types must match the header exactly. ----

const STATUS_OK: i32 = 0;
const STATUS_INVALID_ARGUMENT: i32 = 1;

#[repr(C)]
struct BackendConfig {
    size: usize,
    gpu: i32, // -1 = CPU
}

#[repr(C)]
struct ModelConfig {
    size: usize,
    path: *const c_char,
    n_ctx: i32, // 0 = default
}

#[repr(C)]
struct TranslatorConfig {
    size: usize,
    backend: *const BackendConfig,
    model: *const ModelConfig,
    generation: *const core::ffi::c_void, // NULL = default max_new_tokens
    pool: *const core::ffi::c_void,       // NULL = default contexts
}

#[repr(C)]
struct NativeTranslator {
    _opaque: [u8; 0],
}
#[repr(C)]
struct NativeResult {
    _opaque: [u8; 0],
}

type CreateFn = unsafe extern "C" fn(*const TranslatorConfig, *mut *mut NativeTranslator) -> i32;
type DestroyFn = unsafe extern "C" fn(*mut NativeTranslator);
type TranslateFn = unsafe extern "C" fn(
    *mut NativeTranslator,
    *const *const c_char,
    usize,
    *const c_char,
    *const c_char,
    *mut *mut NativeResult,
) -> i32;
type ResultCountFn = unsafe extern "C" fn(*const NativeResult) -> usize;
type ResultTextFn = unsafe extern "C" fn(*const NativeResult, usize) -> *const c_char;
type ResultDestroyFn = unsafe extern "C" fn(*mut NativeResult);

/// Function pointers resolved from the library; valid while `_library` is loaded.
struct Api {
    create: CreateFn,
    destroy: DestroyFn,
    translate: TranslateFn,
    result_count: ResultCountFn,
    result_text: ResultTextFn,
    result_destroy: ResultDestroyFn,
    // Declared last so it is dropped after every handle that uses these pointers.
    _library: Library,
}

impl Api {
    fn load(library_dir: &std::path::Path) -> Result<Self, TranslatorError> {
        let path = library_dir.join(LIBRARY_FILENAME);
        let library = open_library(&path).map_err(|_| TranslatorError::LibraryUnavailable)?;
        // SAFETY: the names and signatures below follow nmt.h of the pinned NeMo-Speech.cpp
        // release; each pointer is copied out while `library` stays alive in the struct.
        unsafe {
            macro_rules! symbol {
                ($name:literal) => {
                    *library
                        .get($name)
                        .map_err(|_| TranslatorError::LibraryUnavailable)?
                };
            }
            Ok(Self {
                create: symbol!(b"nemo_speech_nmt_create\0"),
                destroy: symbol!(b"nemo_speech_nmt_destroy\0"),
                translate: symbol!(b"nemo_speech_nmt_translate\0"),
                result_count: symbol!(b"nemo_speech_nmt_result_count\0"),
                result_text: symbol!(b"nemo_speech_nmt_result_text\0"),
                result_destroy: symbol!(b"nemo_speech_nmt_result_destroy\0"),
                _library: library,
            })
        }
    }
}

/// Where the NMT library and model are found.
#[derive(Debug, Clone)]
pub struct RivaPaths {
    pub library_dir: PathBuf,
    pub model: PathBuf,
}

impl RivaPaths {
    pub fn resolve() -> Self {
        Self {
            library_dir: crate::native::nemo_library_dir(),
            model: RIVA_TRANSLATE_Q8.path_in(&crate::models::models_root()),
        }
    }

    pub fn translation_available(&self) -> bool {
        self.library_dir.join(LIBRARY_FILENAME).is_file()
            && RIVA_TRANSLATE_Q8.verify(&self.model).is_ok()
    }
}

/// A loaded translator. The C API is thread-safe, but one caption session owns
/// one translator, so `&mut self` keeps the usage single-threaded.
pub struct NativeRivaTranslator {
    translator: *mut NativeTranslator,
    api: Api,
}

// SAFETY: the handle is owned exclusively by this value; the library allows
// calls from any thread.
unsafe impl Send for NativeRivaTranslator {}

impl NativeRivaTranslator {
    /// Loads the model on the GPU (Vulkan device 0) and falls back to the CPU.
    pub fn load(paths: &RivaPaths) -> Result<Self, TranslatorError> {
        RIVA_TRANSLATE_Q8
            .verify(&paths.model)
            .map_err(|_| TranslatorError::ModelUnavailable)?;
        let api = Api::load(&paths.library_dir)?;
        let model_path = paths
            .model
            .to_str()
            .and_then(|path| CString::new(path).ok())
            .ok_or(TranslatorError::ModelUnavailable)?;

        let mut translator = ptr::null_mut();
        for gpu in [0, -1] {
            let backend = BackendConfig {
                size: size_of::<BackendConfig>(),
                gpu,
            };
            let model = ModelConfig {
                size: size_of::<ModelConfig>(),
                path: model_path.as_ptr(),
                n_ctx: 0, // library default
            };
            let config = TranslatorConfig {
                size: size_of::<TranslatorConfig>(),
                backend: &backend,
                model: &model,
                generation: ptr::null(),
                pool: ptr::null(),
            };
            // SAFETY: every config pointer refers to a live local; NULL sections take
            // library defaults.
            let status = unsafe { (api.create)(&config, &mut translator) };
            if status == STATUS_OK && !translator.is_null() {
                break;
            }
            translator = ptr::null_mut();
        }
        if translator.is_null() {
            return Err(TranslatorError::ModelUnavailable);
        }
        Ok(Self { translator, api })
    }

    fn translate_tag(
        &mut self,
        text: &str,
        source: &str,
        target: &str,
    ) -> Result<String, TranslatorError> {
        let text = CString::new(text).map_err(|_| TranslatorError::TranslationFailed)?;
        let source = CString::new(source).map_err(|_| TranslatorError::TranslationFailed)?;
        let target = CString::new(target).map_err(|_| TranslatorError::TranslationFailed)?;
        let texts = [text.as_ptr()];
        let mut result = ptr::null_mut();
        // SAFETY: the pointers are valid for the call; a non-null result is owned by us
        // and destroyed below.
        let status = unsafe {
            (self.api.translate)(
                self.translator,
                texts.as_ptr(),
                texts.len(),
                source.as_ptr(),
                target.as_ptr(),
                &mut result,
            )
        };
        if status == STATUS_INVALID_ARGUMENT {
            return Err(TranslatorError::UnsupportedLanguagePair);
        }
        if status != STATUS_OK || result.is_null() {
            return Err(TranslatorError::TranslationFailed);
        }
        // SAFETY: `result` is valid until destroyed; the text is copied first.
        unsafe {
            let translated = ((self.api.result_count)(result) == 1)
                .then(|| (self.api.result_text)(result, 0))
                .filter(|text| !text.is_null())
                .map(|text| CStr::from_ptr(text).to_string_lossy().into_owned());
            (self.api.result_destroy)(result);
            translated.ok_or(TranslatorError::TranslationFailed)
        }
    }
}

impl Translator for NativeRivaTranslator {
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
        // Every pair the model supports includes English; other pairs hop through it.
        if source != ENGLISH && target != ENGLISH {
            let english = self.translate_tag(text, &source, ENGLISH)?;
            return self.translate_tag(&english, ENGLISH, &target);
        }
        self.translate_tag(text, &source, &target)
    }
}

impl Drop for NativeRivaTranslator {
    fn drop(&mut self) {
        // SAFETY: the handle was created by this library and is released once; `api`
        // (and the library) is dropped only after this.
        unsafe { (self.api.destroy)(self.translator) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_structs_match_the_c_layout() {
        // nmt.h on x64: size_t is 8 bytes and every field is pointer- or int-sized.
        assert_eq!(size_of::<BackendConfig>(), 16);
        assert_eq!(size_of::<ModelConfig>(), 24);
        assert_eq!(size_of::<TranslatorConfig>(), 40);
    }

    #[test]
    fn recognizer_languages_map_to_model_tags() {
        assert_eq!(model_tag("en-US").as_deref(), Some("en"));
        assert_eq!(model_tag("en-GB").as_deref(), Some("en"));
        assert_eq!(model_tag("pt-BR").as_deref(), Some("pt-br"));
        assert_eq!(model_tag("pt-PT").as_deref(), Some("pt-br"));
        assert_eq!(model_tag("es-ES").as_deref(), Some("es"));
        assert_eq!(model_tag("ru-RU").as_deref(), Some("ru"));
        assert_eq!(model_tag(""), None);
        assert_eq!(model_tag("latin"), None);
        assert_eq!(model_tag("1a"), None);
    }

    #[test]
    fn missing_library_or_model_is_reported_without_loading_anything() {
        let empty = std::env::temp_dir().join("anycaptions-missing-nmt");
        let paths = RivaPaths {
            library_dir: empty.clone(),
            model: empty.join("missing.gguf"),
        };
        assert!(matches!(
            NativeRivaTranslator::load(&paths),
            Err(TranslatorError::ModelUnavailable)
        ));
        assert!(!paths.translation_available());
    }
}
