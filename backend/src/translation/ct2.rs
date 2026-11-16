//! The local `opus_ct2_c` wrapper (CTranslate2 int8 on the CPU plus
//! SentencePiece), shared by the engines that run on it: `opus` (one model per
//! direction) and `m2m` (one model for every pair). Its source lives in
//! `backend/native/opus-ct2`. The library is loaded at runtime, so builds and
//! unit tests never need it.

use std::{
    ffi::{CStr, CString, c_char},
    path::Path,
    ptr,
    sync::Arc,
};

use libloading::Library;

use super::TranslatorError;
use crate::native::open_library;

#[cfg(windows)]
pub const LIBRARY_FILENAME: &str = "opus_ct2_c.dll";
#[cfg(not(windows))]
pub const LIBRARY_FILENAME: &str = "libopus_ct2_c.so";

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
type TranslateToFn = unsafe extern "C" fn(
    *mut NativeTranslator,
    *const c_char,
    *const c_char,
    *const c_char,
    *mut *mut c_char,
) -> i32;
type ResultDestroyFn = unsafe extern "C" fn(*mut c_char);

/// Function pointers resolved from the library; valid while `_library` is loaded.
pub(super) struct Ct2Library {
    create: CreateFn,
    destroy: DestroyFn,
    translate: TranslateFn,
    /// Added in wrapper 0.2.0 for many-to-many models. `None` with an older
    /// library, which still serves the OPUS models.
    translate_to: Option<TranslateToFn>,
    result_destroy: ResultDestroyFn,
    // Declared last so it is dropped after the pointers above.
    _library: Library,
}

impl Ct2Library {
    pub(super) fn load(library_dir: &Path) -> Result<Arc<Self>, TranslatorError> {
        let path = library_dir.join(LIBRARY_FILENAME);
        let library = open_library(&path).map_err(|_| TranslatorError::LibraryUnavailable)?;
        // SAFETY: the names and signatures below follow opus_ct2_c.h of the
        // wrapper in `backend/native/opus-ct2`; each pointer is copied out
        // while `library` stays alive in the struct.
        unsafe {
            macro_rules! symbol {
                ($name:literal) => {
                    *library
                        .get($name)
                        .map_err(|_| TranslatorError::LibraryUnavailable)?
                };
            }
            Ok(Arc::new(Self {
                create: symbol!(b"opus_ct2_create\0"),
                destroy: symbol!(b"opus_ct2_destroy\0"),
                translate: symbol!(b"opus_ct2_translate\0"),
                translate_to: library
                    .get::<TranslateToFn>(b"opus_ct2_translate_to\0")
                    .ok()
                    .map(|symbol| *symbol),
                result_destroy: symbol!(b"opus_ct2_result_destroy\0"),
                _library: library,
            }))
        }
    }

    /// Whether this build of the wrapper can run many-to-many models.
    pub(super) fn supports_target_tokens(&self) -> bool {
        self.translate_to.is_some()
    }
}

/// One loaded CTranslate2 model with its SentencePiece tokenizers.
pub(super) struct Ct2Model {
    handle: *mut NativeTranslator,
    // Keeps the library loaded for as long as the handle exists.
    library: Arc<Ct2Library>,
}

// SAFETY: the handle is owned exclusively by this value; CTranslate2 allows
// calls from any thread, and every call goes through `&self` on one owner.
unsafe impl Send for Ct2Model {}

impl Ct2Model {
    /// Loads the model in `model_dir` with the given tokenizer files.
    pub(super) fn load(
        library: &Arc<Ct2Library>,
        model_dir: &Path,
        source_spm: &Path,
        target_spm: &Path,
    ) -> Result<Self, TranslatorError> {
        let model_dir = path_cstring(model_dir)?;
        let source_spm = path_cstring(source_spm)?;
        let target_spm = path_cstring(target_spm)?;
        let mut handle = ptr::null_mut();
        // SAFETY: every pointer refers to a live CString; the handle is checked below.
        let status = unsafe {
            (library.create)(
                model_dir.as_ptr(),
                source_spm.as_ptr(),
                target_spm.as_ptr(),
                INTRA_THREADS,
                &mut handle,
            )
        };
        if status != STATUS_OK || handle.is_null() {
            return Err(TranslatorError::ModelUnavailable);
        }
        Ok(Self {
            handle,
            library: Arc::clone(library),
        })
    }

    /// Translates one sentence. `prefix` is the optional token a multi-target
    /// OPUS model expects first in the source (e.g. `>>pob<<`); `""` is none.
    pub(super) fn translate(&self, text: &str, prefix: &str) -> Result<String, TranslatorError> {
        let text = text_cstring(text)?;
        let prefix = text_cstring(prefix)?;
        let mut result: *mut c_char = ptr::null_mut();
        // SAFETY: the pointers are valid for the call; a non-null result is
        // owned by us and released in `take_result`.
        let status = unsafe {
            (self.library.translate)(self.handle, text.as_ptr(), prefix.as_ptr(), &mut result)
        };
        self.take_result(status, result)
    }

    /// Translates one sentence with a many-to-many model: `source_token` and
    /// `target_token` name the languages in the model's vocabulary
    /// (e.g. `__en__`, `__pt__`).
    pub(super) fn translate_to(
        &self,
        text: &str,
        source_token: &str,
        target_token: &str,
    ) -> Result<String, TranslatorError> {
        let translate_to = self
            .library
            .translate_to
            .ok_or(TranslatorError::LibraryUnavailable)?;
        let text = text_cstring(text)?;
        let source_token = text_cstring(source_token)?;
        let target_token = text_cstring(target_token)?;
        let mut result: *mut c_char = ptr::null_mut();
        // SAFETY: as in `translate`.
        let status = unsafe {
            translate_to(
                self.handle,
                text.as_ptr(),
                source_token.as_ptr(),
                target_token.as_ptr(),
                &mut result,
            )
        };
        self.take_result(status, result)
    }

    /// Copies a result string out of the library and releases it.
    fn take_result(&self, status: i32, result: *mut c_char) -> Result<String, TranslatorError> {
        if status == STATUS_INVALID_ARGUMENT {
            return Err(TranslatorError::UnsupportedLanguagePair);
        }
        if status != STATUS_OK || result.is_null() {
            return Err(TranslatorError::TranslationFailed);
        }
        // SAFETY: `result` is a NUL-terminated string owned by us until destroyed.
        unsafe {
            let translated = CStr::from_ptr(result).to_string_lossy().into_owned();
            (self.library.result_destroy)(result);
            Ok(translated)
        }
    }
}

impl Drop for Ct2Model {
    fn drop(&mut self) {
        // SAFETY: the handle was created by this library and is released once;
        // the library is dropped only after this, through the `Arc`.
        unsafe { (self.library.destroy)(self.handle) };
    }
}

fn path_cstring(path: &Path) -> Result<CString, TranslatorError> {
    path.to_str()
        .and_then(|value| CString::new(value).ok())
        .ok_or(TranslatorError::ModelUnavailable)
}

fn text_cstring(text: &str) -> Result<CString, TranslatorError> {
    CString::new(text).map_err(|_| TranslatorError::TranslationFailed)
}

/// Translates `text` one sentence at a time and joins the results. These
/// models are trained on single sentences: given "Can you hear me? I think my
/// microphone is broken." both OPUS-MT and M2M100 returned only the second
/// sentence (measured 2026-11-16), and the translation stage deliberately
/// merges short sentences with their neighbor, so whole questions vanished
/// from translated captions.
pub(super) fn translate_by_sentence(
    text: &str,
    mut translate: impl FnMut(&str) -> Result<String, TranslatorError>,
) -> Result<String, TranslatorError> {
    let pieces = sentence_pieces(text);
    if pieces.len() <= 1 {
        return translate(text);
    }
    let mut out = Vec::with_capacity(pieces.len());
    for piece in pieces {
        out.push(translate(piece)?);
    }
    Ok(out.join(" "))
}

/// Splits on sentence marks followed by whitespace, keeping each mark with its
/// sentence ("Hi! Ok." -> ["Hi!", "Ok."]). Decimals ("2.5") and marks without a
/// following space stay inside their sentence.
fn sentence_pieces(text: &str) -> Vec<&str> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if matches!(ch, '.' | '!' | '?' | '…' | '。' | '！' | '？')
            && chars.peek().is_some_and(|(_, next)| next.is_whitespace())
        {
            let end = index + ch.len_utf8();
            let piece = text[start..end].trim();
            if !piece.is_empty() {
                pieces.push(piece);
            }
            start = end;
        }
    }
    let rest = text[start..].trim();
    if !rest.is_empty() {
        pieces.push(rest);
    }
    pieces
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merged_captions_split_back_into_single_sentences() {
        assert_eq!(
            sentence_pieces("Can you hear me? I think my microphone is broken."),
            ["Can you hear me?", "I think my microphone is broken."]
        );
        assert_eq!(sentence_pieces("Ok. Go!  Wait…"), ["Ok.", "Go!", "Wait…"]);
        assert_eq!(
            sentence_pieces("Version 2.5 is out"),
            ["Version 2.5 is out"]
        );
        assert_eq!(sentence_pieces("no mark at all"), ["no mark at all"]);
        assert!(sentence_pieces("   ").is_empty());
    }

    #[test]
    fn each_sentence_is_translated_on_its_own_and_errors_stop_the_caption() {
        let mut seen = Vec::new();
        let joined = translate_by_sentence("One. Two? Three", |piece| {
            seen.push(piece.to_owned());
            Ok(piece.to_uppercase())
        });
        assert_eq!(joined.as_deref(), Ok("ONE. TWO? THREE"));
        assert_eq!(seen, ["One.", "Two?", "Three"]);

        let failed =
            translate_by_sentence("One. Two.", |_| Err(TranslatorError::TranslationFailed));
        assert_eq!(failed, Err(TranslatorError::TranslationFailed));
    }

    #[test]
    fn a_missing_library_is_reported_without_loading_anything() {
        let empty = std::env::temp_dir().join("anycaptions-missing-ct2");
        assert!(matches!(
            Ct2Library::load(&empty),
            Err(TranslatorError::LibraryUnavailable)
        ));
    }
}
