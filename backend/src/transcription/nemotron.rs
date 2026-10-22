//! NVIDIA Nemotron streaming ASR through the NeMo-Speech.cpp C API (`include/nemo_speech/asr.h`,
//! v0.2.0). The library is loaded at runtime, so builds and unit tests never need it.

use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::{Path, PathBuf},
    ptr,
};

use libloading::Library;

use super::streaming::{RecognizerError, SpeechResult, StreamingRecognizer};
use crate::models::catalog::NEMOTRON_STREAMING_Q8;

#[cfg(windows)]
pub const LIBRARY_FILENAME: &str = "nemo_speech_asr_c.dll";
#[cfg(not(windows))]
pub const LIBRARY_FILENAME: &str = "libnemo_speech_asr_c.so";
// Audio pushed to the stream is already 16 kHz mono; 0 tells the library not to resample.
const MODEL_RATE: i32 = 0;
// Model-based detection; the product shows English and Portuguese speech.
const LANGUAGE_AUTO: &CStr = c"auto";

// ---- C ABI mirror of asr.h. Field order and types must match the header exactly. ----

const STATUS_OK: i32 = 0;

#[repr(C)]
struct BackendConfig {
    size: usize,
    gpu: i32, // -1 = CPU
}

#[repr(C)]
struct ModelConfig {
    size: usize,
    path: *const c_char,
    name: *const c_char,
}

#[repr(C)]
struct EndpointingConfig {
    size: usize,
    enable: bool,
    vad_based: bool,
    stop_history_eou_ms: i32,
}

#[repr(C)]
struct RecognizerConfig {
    size: usize,
    backend: *const BackendConfig,
    model: *const ModelConfig,
    streaming: *const c_void,
    decoder: *const c_void,
    vad: *const c_void,
    endpointing: *const EndpointingConfig,
    postproc: *const c_void,
    diar: *const c_void,
    batching: *const c_void,
}

#[repr(C)]
struct RecognitionOptions {
    size: usize,
    request_id: *const c_char,
    language_code: *const c_char,
    interim_results: bool,
    enable_word_time_offsets: bool,
    enable_automatic_punctuation: bool,
    verbatim_transcripts: bool,
    profanity_filter: bool,
    stop_history_eou_ms: i32,
    speech_contexts: *const c_void,
    speech_context_count: usize,
    max_alternatives: i32,
    enable_speaker_diarization: bool,
    max_speaker_count: i32,
}

#[repr(C)]
struct Recognizer {
    _opaque: [u8; 0],
}
#[repr(C)]
struct Stream {
    _opaque: [u8; 0],
}
#[repr(C)]
struct NativeResult {
    _opaque: [u8; 0],
}

type OptionsDefaultFn = unsafe extern "C" fn() -> RecognitionOptions;
type CreateFn = unsafe extern "C" fn(*const RecognizerConfig, *mut *mut Recognizer) -> i32;
type DestroyFn = unsafe extern "C" fn(*mut Recognizer);
type StreamingRecognizeFn =
    unsafe extern "C" fn(*mut Recognizer, *const RecognitionOptions, *mut *mut Stream) -> i32;
type PushFn = unsafe extern "C" fn(*mut Stream, *const f32, usize, i32) -> i32;
type FinishFn = unsafe extern "C" fn(*mut Stream) -> i32;
type NextFn = unsafe extern "C" fn(*mut Stream, *mut *mut NativeResult) -> i32;
type CloseFn = unsafe extern "C" fn(*mut Stream);
type IsFinalFn = unsafe extern "C" fn(*const NativeResult) -> bool;
type TranscriptFn = unsafe extern "C" fn(*const NativeResult, usize) -> *const c_char;
type LanguageCountFn = unsafe extern "C" fn(*const NativeResult, usize) -> usize;
type LanguageCodeFn = unsafe extern "C" fn(*const NativeResult, usize, usize) -> *const c_char;
type ResultDestroyFn = unsafe extern "C" fn(*mut NativeResult);

/// Function pointers resolved from the library; valid while `_library` is loaded.
struct Api {
    options_default: OptionsDefaultFn,
    create: CreateFn,
    destroy: DestroyFn,
    streaming_recognize: StreamingRecognizeFn,
    push: PushFn,
    finish: FinishFn,
    next: NextFn,
    close: CloseFn,
    is_final: IsFinalFn,
    transcript: TranscriptFn,
    language_count: LanguageCountFn,
    language_code: LanguageCodeFn,
    result_destroy: ResultDestroyFn,
    // Declared last so it is dropped after every handle that uses these pointers.
    _library: Library,
}

impl Api {
    fn load(library_dir: &Path) -> Result<Self, RecognizerError> {
        let path = library_dir.join(LIBRARY_FILENAME);
        let library = open_library(&path).map_err(|_| RecognizerError::LibraryUnavailable)?;
        // SAFETY: the names and signatures below follow asr.h of the pinned NeMo-Speech.cpp
        // release; each pointer is copied out while `library` stays alive in the struct.
        unsafe {
            macro_rules! symbol {
                ($name:literal) => {
                    *library
                        .get($name)
                        .map_err(|_| RecognizerError::LibraryUnavailable)?
                };
            }
            Ok(Self {
                options_default: symbol!(b"nemo_speech_asr_recognition_options_default\0"),
                create: symbol!(b"nemo_speech_asr_create\0"),
                destroy: symbol!(b"nemo_speech_asr_destroy\0"),
                streaming_recognize: symbol!(b"nemo_speech_asr_streaming_recognize\0"),
                push: symbol!(b"nemo_speech_asr_stream_push_f32\0"),
                finish: symbol!(b"nemo_speech_asr_stream_finish\0"),
                next: symbol!(b"nemo_speech_asr_stream_next\0"),
                close: symbol!(b"nemo_speech_asr_stream_close\0"),
                is_final: symbol!(b"nemo_speech_asr_result_is_final\0"),
                transcript: symbol!(b"nemo_speech_asr_result_transcript\0"),
                language_count: symbol!(b"nemo_speech_asr_result_language_count\0"),
                language_code: symbol!(b"nemo_speech_asr_result_language_code\0"),
                result_destroy: symbol!(b"nemo_speech_asr_result_destroy\0"),
                _library: library,
            })
        }
    }
}

#[cfg(windows)]
fn open_library(path: &Path) -> Result<Library, libloading::Error> {
    use libloading::os::windows::{LOAD_WITH_ALTERED_SEARCH_PATH, Library as WindowsLibrary};
    // The library's dependencies (ggml*.dll, nemo_speech_asr.dll) live next to it, not next to
    // the executable; this flag makes Windows resolve them from the library's own folder.
    // SAFETY: loading runs the library's initializers; it is the pinned NeMo-Speech.cpp build.
    unsafe { WindowsLibrary::load_with_flags(path, LOAD_WITH_ALTERED_SEARCH_PATH).map(Into::into) }
}

#[cfg(not(windows))]
fn open_library(path: &Path) -> Result<Library, libloading::Error> {
    // SAFETY: loading runs the library's initializers; it is the pinned NeMo-Speech.cpp build.
    unsafe { Library::new(path) }
}

/// Where the NeMo-Speech library and model are found.
#[derive(Debug, Clone)]
pub struct NemotronPaths {
    pub library_dir: PathBuf,
    pub model: PathBuf,
}

/// A loaded recognizer with one open stream. NeMo detects speech and utterance ends itself.
pub struct NativeNemotron {
    recognizer: *mut Recognizer,
    stream: *mut Stream,
    api: Api,
}

// SAFETY: the handles are owned exclusively by this value and only used through `&mut self`,
// so they are never accessed from two threads at once; NeMo does not bind them to a thread.
unsafe impl Send for NativeNemotron {}

impl NativeNemotron {
    /// Loads the model on the GPU (Vulkan device 0) and falls back to the CPU.
    /// `forced_language` restricts recognition to one language from the model's prompt
    /// dictionary (e.g. `en-US`); `None` keeps auto-detection.
    pub fn load(
        paths: &NemotronPaths,
        forced_language: Option<&str>,
    ) -> Result<Self, RecognizerError> {
        NEMOTRON_STREAMING_Q8
            .verify(&paths.model)
            .map_err(|_| RecognizerError::ModelUnavailable)?;
        let language = match forced_language {
            Some(code) => CString::new(code).map_err(|_| RecognizerError::RecognitionFailed)?,
            None => LANGUAGE_AUTO.to_owned(),
        };
        let api = Api::load(&paths.library_dir)?;
        let model_path = paths
            .model
            .to_str()
            .and_then(|path| CString::new(path).ok())
            .ok_or(RecognizerError::ModelUnavailable)?;

        let mut recognizer = ptr::null_mut();
        for gpu in [0, -1] {
            recognizer = create_recognizer(&api, &model_path, gpu);
            if !recognizer.is_null() {
                break;
            }
        }
        if recognizer.is_null() {
            return Err(RecognizerError::ModelUnavailable);
        }

        // SAFETY: the defaults come from the library and are fully initialized; only plain
        // fields are changed, and the strings outlive the call.
        let stream = unsafe {
            let mut options = (api.options_default)();
            options.language_code = language.as_ptr();
            options.interim_results = true;
            options.enable_automatic_punctuation = true;
            let mut stream = ptr::null_mut();
            let status = (api.streaming_recognize)(recognizer, &options, &mut stream);
            if status != STATUS_OK || stream.is_null() {
                (api.destroy)(recognizer);
                return Err(RecognizerError::RecognitionFailed);
            }
            stream
        };
        Ok(Self {
            recognizer,
            stream,
            api,
        })
    }

    fn drain(&mut self) -> Result<Vec<SpeechResult>, RecognizerError> {
        let mut results = Vec::new();
        loop {
            let mut result = ptr::null_mut();
            // SAFETY: `stream` is open; a non-null result is owned by us and destroyed below.
            let status = unsafe { (self.api.next)(self.stream, &mut result) };
            if status != STATUS_OK {
                return Err(RecognizerError::RecognitionFailed);
            }
            if result.is_null() {
                return Ok(results);
            }
            // SAFETY: `result` is valid until destroyed; transcript and language are copied first.
            unsafe {
                let text = (self.api.transcript)(result, 0);
                let text = if text.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(text).to_string_lossy().into_owned()
                };
                let is_final = (self.api.is_final)(result);
                let language = ((self.api.language_count)(result, 0) > 0)
                    .then(|| (self.api.language_code)(result, 0, 0))
                    .filter(|code| !code.is_null())
                    .map(|code| CStr::from_ptr(code).to_string_lossy().into_owned())
                    .filter(|code| !code.is_empty());
                (self.api.result_destroy)(result);
                results.push(SpeechResult {
                    text,
                    is_final,
                    language,
                });
            }
        }
    }
}

fn create_recognizer(api: &Api, model_path: &CStr, gpu: i32) -> *mut Recognizer {
    let backend = BackendConfig {
        size: size_of::<BackendConfig>(),
        gpu,
    };
    let model = ModelConfig {
        size: size_of::<ModelConfig>(),
        path: model_path.as_ptr(),
        name: ptr::null(),
    };
    // Finalize each utterance on silence so captions are committed during pauses.
    let endpointing = EndpointingConfig {
        size: size_of::<EndpointingConfig>(),
        enable: true,
        vad_based: false,
        stop_history_eou_ms: 0, // library default
    };
    let config = RecognizerConfig {
        size: size_of::<RecognizerConfig>(),
        backend: &backend,
        model: &model,
        streaming: ptr::null(),
        decoder: ptr::null(),
        vad: ptr::null(),
        endpointing: &endpointing,
        postproc: ptr::null(),
        diar: ptr::null(),
        batching: ptr::null(),
    };
    let mut recognizer = ptr::null_mut();
    // SAFETY: every config pointer refers to a live local; NULL sections take library defaults.
    let status = unsafe { (api.create)(&config, &mut recognizer) };
    if status == STATUS_OK {
        recognizer
    } else {
        ptr::null_mut()
    }
}

impl StreamingRecognizer for NativeNemotron {
    fn push(&mut self, samples: &[f32]) -> Result<Vec<SpeechResult>, RecognizerError> {
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err(RecognizerError::InvalidAudio);
        }
        // SAFETY: `stream` is open and the slice is valid for `samples.len()` floats.
        let status =
            unsafe { (self.api.push)(self.stream, samples.as_ptr(), samples.len(), MODEL_RATE) };
        if status != STATUS_OK {
            return Err(RecognizerError::RecognitionFailed);
        }
        self.drain()
    }

    fn finish(&mut self) -> Result<Vec<SpeechResult>, RecognizerError> {
        // SAFETY: `stream` is open; finishing only flushes buffered audio.
        if unsafe { (self.api.finish)(self.stream) } != STATUS_OK {
            return Err(RecognizerError::RecognitionFailed);
        }
        self.drain()
    }
}

impl Drop for NativeNemotron {
    fn drop(&mut self) {
        // SAFETY: both handles were created by this library and are released once, stream
        // first; `api` (and the library) is dropped only after this.
        unsafe {
            (self.api.close)(self.stream);
            (self.api.destroy)(self.recognizer);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_structs_match_the_c_layout() {
        // asr.h on x64: size_t is 8 bytes; bools are 1 byte, padded before the next int32.
        assert_eq!(size_of::<BackendConfig>(), 16);
        assert_eq!(size_of::<ModelConfig>(), 24);
        assert_eq!(size_of::<EndpointingConfig>(), 16);
        assert_eq!(size_of::<RecognizerConfig>(), 80);
        assert_eq!(size_of::<RecognitionOptions>(), 72);
    }

    #[test]
    fn missing_library_or_model_is_reported_without_loading_anything() {
        let empty = std::env::temp_dir().join("anycaptions-missing-nemo");
        let paths = NemotronPaths {
            library_dir: empty.clone(),
            model: empty.join("missing.gguf"),
        };
        assert!(matches!(
            NativeNemotron::load(&paths, None),
            Err(RecognizerError::ModelUnavailable)
        ));
        assert!(matches!(
            Api::load(&empty),
            Err(RecognizerError::LibraryUnavailable)
        ));
    }
}
