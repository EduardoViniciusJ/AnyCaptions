//! NVIDIA Nemotron streaming ASR through the NeMo-Speech.cpp C API (`include/nemo_speech/asr.h`,
//! v0.3.0; the structs are size-prefixed and append-only, so the v0.2.0 library also
//! accepts them). The library is loaded at runtime, so builds and unit tests never need it.

use std::{
    ffi::{CStr, CString, c_char, c_void},
    path::{Path, PathBuf},
    ptr,
    sync::{Arc, Mutex},
};

use libloading::Library;

use super::{
    diarizer::{DiarizerModel, DiarizerStream, InterimWords},
    streaming::{RecognizerError, SpeechResult, StreamingRecognizer, TaggedWord},
    tuning::RecognizerTuning,
};
use crate::{models::catalog::NEMOTRON_STREAMING_Q8, native::open_library};

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
struct StreamingConfig {
    size: usize,
    chunk_size: f32,         // seconds; CTC only, 0 = default
    ctc_left_padding: f32,   // CTC only
    ctc_right_padding: f32,  // CTC only
    rnnt_right_context: i32, // encoder frames; -1 = model maximum
}

#[repr(C)]
struct RecognizerConfig {
    size: usize,
    backend: *const BackendConfig,
    model: *const ModelConfig,
    streaming: *const StreamingConfig,
    decoder: *const c_void,
    vad: *const c_void,
    endpointing: *const EndpointingConfig,
    postproc: *const c_void,
    // Speaker diarization sidecar: unused, the standalone diarizer serves instead.
    diar: *const c_void,
    batching: *const c_void,
    /// Speech-LLM transcription (v0.3.0); NULL = the ordinary ASR model.
    salm: *const c_void,
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
type WordCountFn = unsafe extern "C" fn(*const NativeResult, usize) -> usize;
type WordTextFn = unsafe extern "C" fn(*const NativeResult, usize, usize) -> *const c_char;
type WordTimeFn = unsafe extern "C" fn(*const NativeResult, usize, usize) -> i32;
type AudioProcessedFn = unsafe extern "C" fn(*const NativeResult) -> f32;
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
    word_count: WordCountFn,
    word_text: WordTextFn,
    word_start_time: WordTimeFn,
    word_end_time: WordTimeFn,
    audio_processed: AudioProcessedFn,
    result_destroy: ResultDestroyFn,
    // Kept loaded for the rest of the process (see `load_library`).
    _library: &'static Library,
}

/// Opens the NeMo-Speech library once per process and never unloads it. The
/// ggml runtime inside it installs a C++ terminate handler from a static
/// initializer; unloading and loading the DLL again (last model dropped, next
/// model created) re-runs that initializer, which the ggml in NeMo-Speech.cpp
/// v0.3.0 rejects with a fatal assertion. Inference libraries are not meant
/// to be cycled anyway (earlier reloads also leaked GPU memory).
pub(super) fn load_library(path: &Path) -> Result<&'static Library, RecognizerError> {
    static LIBRARIES: Mutex<Vec<(PathBuf, &'static Library)>> = Mutex::new(Vec::new());
    let mut loaded = LIBRARIES
        .lock()
        .map_err(|_| RecognizerError::LibraryUnavailable)?;
    if let Some((_, library)) = loaded.iter().find(|(loaded, _)| loaded == path) {
        return Ok(library);
    }
    let library: &'static Library = Box::leak(Box::new(
        open_library(path).map_err(|_| RecognizerError::LibraryUnavailable)?,
    ));
    loaded.push((path.to_path_buf(), library));
    Ok(library)
}

impl Api {
    fn load(library_dir: &Path) -> Result<Self, RecognizerError> {
        let path = library_dir.join(LIBRARY_FILENAME);
        let library = load_library(&path)?;
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
                word_count: symbol!(b"nemo_speech_asr_result_word_count\0"),
                word_text: symbol!(b"nemo_speech_asr_result_word_text\0"),
                word_start_time: symbol!(b"nemo_speech_asr_result_word_start_time\0"),
                word_end_time: symbol!(b"nemo_speech_asr_result_word_end_time\0"),
                audio_processed: symbol!(b"nemo_speech_asr_result_audio_processed\0"),
                result_destroy: symbol!(b"nemo_speech_asr_result_destroy\0"),
                _library: library,
            })
        }
    }
}

/// Where the NeMo-Speech library and models are found.
#[derive(Debug, Clone)]
pub struct NemotronPaths {
    pub library_dir: PathBuf,
    pub model: PathBuf,
    /// The optional Sortformer diarization GGUF (speaker labels).
    pub diarization: PathBuf,
}

/// The loaded model and its recognizer. Loading takes seconds and holds the
/// model in (V)RAM, so the app shares one instance across caption sessions
/// instead of reloading per session (repeated native reloads also leaked GPU
/// memory until allocation failed). Per-session choices — language,
/// endpointing and whether to tag speakers — travel in each stream's options;
/// the diarizer itself is attached here, at load time.
pub struct NemotronModel {
    recognizer: *mut Recognizer,
    api: Api,
    /// The standalone diarizer loaded next to the recognizer (speaker labels).
    diarizer: Option<Arc<DiarizerModel>>,
}

// SAFETY: asr.h supports submitting work to one recognizer from multiple
// threads (its batching section); handles are not bound to a thread. Sessions
// are serialized by the session manager anyway.
unsafe impl Send for NemotronModel {}
unsafe impl Sync for NemotronModel {}

impl NemotronModel {
    /// Loads the model on the GPU (Vulkan device 0) and falls back to the CPU.
    /// `rnnt_right_context` picks the cache-aware lookahead at load time
    /// (see [`super::tuning::RecognizerTuning::rnnt_right_context`]).
    /// `diarization` also loads the standalone Sortformer diarizer (verified
    /// against the catalog first) so streams may ask for word-level speaker
    /// tags; it fails with [`RecognizerError::DiarizationUnavailable`] rather
    /// than silently loading without it, so the caller can decide to fall back.
    pub fn load(
        paths: &NemotronPaths,
        rnnt_right_context: Option<i32>,
        diarization: bool,
    ) -> Result<Arc<Self>, RecognizerError> {
        NEMOTRON_STREAMING_Q8
            .verify(&paths.model)
            .map_err(|_| RecognizerError::ModelUnavailable)?;
        // The diarizer goes first: its failure must not cost a recognizer load.
        let diarizer = if diarization {
            Some(DiarizerModel::load(paths)?)
        } else {
            None
        };
        let api = Api::load(&paths.library_dir)?;
        let model_path = paths
            .model
            .to_str()
            .and_then(|path| CString::new(path).ok())
            .ok_or(RecognizerError::ModelUnavailable)?;

        let mut recognizer = ptr::null_mut();
        for gpu in [0, -1] {
            recognizer = create_recognizer(&api, &model_path, gpu, rnnt_right_context);
            if !recognizer.is_null() {
                if gpu < 0 {
                    // T07.1: CPU recovery must be explicit, never silent.
                    eprintln!("[asr] gpu unavailable, model loaded on cpu");
                }
                break;
            }
        }
        if recognizer.is_null() {
            return Err(RecognizerError::ModelUnavailable);
        }
        Ok(Arc::new(Self {
            recognizer,
            api,
            diarizer,
        }))
    }

    /// Whether the diarizer is loaded, i.e. streams opened over this model
    /// may request speaker tags.
    pub fn has_diarization(&self) -> bool {
        self.diarizer.is_some()
    }
}

impl Drop for NemotronModel {
    fn drop(&mut self) {
        // SAFETY: every stream keeps an `Arc` to this model, so all streams are
        // closed before the recognizer is destroyed; `api` (and the library) is
        // dropped only after this.
        unsafe { (self.api.destroy)(self.recognizer) };
    }
}

/// One open stream over the shared model. NeMo detects speech and utterance ends itself.
pub struct NativeNemotron {
    stream: *mut Stream,
    model: Arc<NemotronModel>,
    /// The diarizer stream fed in lockstep when speaker tags were requested;
    /// `None` after it fails mid-session (captions then continue untagged).
    diarizer: Option<DiarizerStream>,
    /// The utterance being decoded, for provisional tags on interim results.
    interim: InterimWords,
}

// SAFETY: the stream is owned exclusively by this value and only used through `&mut self`,
// so it is never accessed from two threads at once; NeMo does not bind it to a thread.
unsafe impl Send for NativeNemotron {}

impl NativeNemotron {
    /// Loads a dedicated model and opens one stream; prefer [`NativeNemotron::open`]
    /// with a shared [`NemotronModel`] when more than one session may run.
    /// `speaker_labels` attaches the diarizer and tags every word with a speaker.
    pub fn load(
        paths: &NemotronPaths,
        forced_language: Option<&str>,
        tuning: RecognizerTuning,
        speaker_labels: bool,
    ) -> Result<Self, RecognizerError> {
        Self::open(
            NemotronModel::load(paths, tuning.rnnt_right_context, speaker_labels)?,
            forced_language,
            tuning,
            speaker_labels,
        )
    }

    /// Opens a recognition stream over an already loaded model.
    /// `forced_language` restricts recognition to one language from the model's prompt
    /// dictionary (e.g. `en-US`); `None` keeps auto-detection. `tuning` adjusts
    /// endpointing per caption mode (see `super::tuning`). `speaker_labels`
    /// requests per-word speaker tags; it needs a model loaded with the
    /// diarizer, otherwise the stream fails to open (the factory never asks
    /// for tags on a model without it).
    pub fn open(
        model: Arc<NemotronModel>,
        forced_language: Option<&str>,
        tuning: RecognizerTuning,
        speaker_labels: bool,
    ) -> Result<Self, RecognizerError> {
        let diarizer = if speaker_labels {
            let diarizer = model
                .diarizer
                .as_ref()
                .ok_or(RecognizerError::DiarizationUnavailable)?;
            Some(DiarizerStream::open(Arc::clone(diarizer))?)
        } else {
            None
        };
        let language = match forced_language {
            Some(code) => CString::new(code).map_err(|_| RecognizerError::RecognitionFailed)?,
            None => LANGUAGE_AUTO.to_owned(),
        };
        // SAFETY: the defaults come from the library and are fully initialized; only plain
        // fields are changed, and the strings outlive the call.
        let stream = unsafe {
            let mut options = (model.api.options_default)();
            options.language_code = language.as_ptr();
            options.interim_results = true;
            options.enable_automatic_punctuation = true;
            // Per-session endpointing; `<= 0` keeps the library default (asr.h).
            options.stop_history_eou_ms = tuning.stop_history_eou_ms;
            if speaker_labels {
                // Word times are what the final attribution needs; the library
                // only builds the per-word list when offsets are requested.
                // Plain streams keep it off, so their results are byte-for-byte
                // what they were before.
                options.enable_word_time_offsets = true;
            }
            let mut stream = ptr::null_mut();
            let status = (model.api.streaming_recognize)(model.recognizer, &options, &mut stream);
            if status != STATUS_OK || stream.is_null() {
                return Err(RecognizerError::RecognitionFailed);
            }
            stream
        };
        Ok(Self {
            stream,
            model,
            diarizer,
            interim: InterimWords::default(),
        })
    }

    /// Feeds the diarizer the same audio as the recognizer. A failing
    /// diarizer is dropped: the words then come back untagged and the overlay
    /// shows plain captions instead of ending the session.
    fn feed_diarizer(&mut self, samples: &[f32]) {
        if let Some(diarizer) = self.diarizer.as_mut()
            && diarizer.push(samples).is_err()
        {
            eprintln!("[diar] diarizer stream failed, captions continue without speaker labels");
            self.diarizer = None;
        }
    }

    /// Words of a final result with their exact times, attributed on the
    /// diarizer's timeline.
    ///
    /// # Safety
    /// `result` must be a live result handle from this stream.
    unsafe fn final_words(&self, result: *const NativeResult) -> Vec<TaggedWord> {
        let Some(diarizer) = self.diarizer.as_ref() else {
            return Vec::new();
        };
        let api = &self.model.api;
        // SAFETY: the caller guarantees a live result; indices stay below the count.
        unsafe {
            let count = (api.word_count)(result, 0);
            (0..count)
                .map(|index| {
                    let word = (api.word_text)(result, 0, index);
                    let text = if word.is_null() {
                        String::new()
                    } else {
                        CStr::from_ptr(word).to_string_lossy().into_owned()
                    };
                    let start = (api.word_start_time)(result, 0, index) as f32;
                    let end = (api.word_end_time)(result, 0, index) as f32;
                    TaggedWord {
                        text,
                        speaker: diarizer.timeline().speaker_for_word(start, end),
                    }
                })
                .collect()
        }
    }

    fn drain(&mut self) -> Result<Vec<SpeechResult>, RecognizerError> {
        let mut results = Vec::new();
        loop {
            let mut result = ptr::null_mut();
            // SAFETY: `stream` is open; a non-null result is owned by us and destroyed below.
            let status = unsafe { (self.model.api.next)(self.stream, &mut result) };
            if status != STATUS_OK {
                return Err(RecognizerError::RecognitionFailed);
            }
            if result.is_null() {
                return Ok(results);
            }
            // SAFETY: `result` is valid until destroyed; transcript, language and words are
            // copied out before it is destroyed.
            unsafe {
                let api = &self.model.api;
                let text = (api.transcript)(result, 0);
                let text = if text.is_null() {
                    String::new()
                } else {
                    CStr::from_ptr(text).to_string_lossy().into_owned()
                };
                let is_final = (api.is_final)(result);
                let language = ((api.language_count)(result, 0) > 0)
                    .then(|| (api.language_code)(result, 0, 0))
                    .filter(|code| !code.is_null())
                    .map(|code| CStr::from_ptr(code).to_string_lossy().into_owned())
                    .filter(|code| !code.is_empty());
                let words = match self.diarizer.as_ref() {
                    None => Vec::new(),
                    Some(_) if is_final => {
                        self.interim.clear();
                        self.final_words(result)
                    }
                    Some(diarizer) => {
                        // Interim words have no times: place them behind the
                        // decode position and tag them provisionally.
                        let decoded_ms = (api.audio_processed)(result) * 1000.0;
                        self.interim.update(&text, decoded_ms);
                        self.interim.tagged(diarizer.timeline())
                    }
                };
                (api.result_destroy)(result);
                results.push(SpeechResult {
                    text,
                    is_final,
                    language,
                    words,
                });
            }
        }
    }
}

fn create_recognizer(
    api: &Api,
    model_path: &CStr,
    gpu: i32,
    rnnt_right_context: Option<i32>,
) -> *mut Recognizer {
    let backend = BackendConfig {
        size: size_of::<BackendConfig>(),
        gpu,
    };
    let model = ModelConfig {
        size: size_of::<ModelConfig>(),
        path: model_path.as_ptr(),
        name: ptr::null(),
    };
    // Only built when a mode asks for a non-default lookahead; NULL keeps the
    // library's low-latency preset (right context 1). The CTC fields are not
    // used by this RNNT model but are validated at creation, so they carry the
    // library defaults (runner.h of the pinned build).
    let streaming = rnnt_right_context.map(|frames| StreamingConfig {
        size: size_of::<StreamingConfig>(),
        chunk_size: 0.16,
        ctc_left_padding: 1.92,
        ctc_right_padding: 1.92,
        rnnt_right_context: frames,
    });
    // Finalize each utterance on silence so captions are committed during pauses.
    // The silence threshold stays at the library default here; each stream may
    // shorten it through its recognition options (see `NativeNemotron::open`).
    let endpointing = EndpointingConfig {
        size: size_of::<EndpointingConfig>(),
        enable: true,
        vad_based: false,
        stop_history_eou_ms: 0,
    };
    let config = RecognizerConfig {
        size: size_of::<RecognizerConfig>(),
        backend: &backend,
        model: &model,
        streaming: streaming
            .as_ref()
            .map_or(ptr::null(), |config| config as *const StreamingConfig),
        decoder: ptr::null(),
        vad: ptr::null(),
        endpointing: &endpointing,
        postproc: ptr::null(),
        diar: ptr::null(),
        batching: ptr::null(),
        salm: ptr::null(),
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
        let status = unsafe {
            (self.model.api.push)(self.stream, samples.as_ptr(), samples.len(), MODEL_RATE)
        };
        if status != STATUS_OK {
            return Err(RecognizerError::RecognitionFailed);
        }
        self.feed_diarizer(samples);
        self.drain()
    }

    fn finish(&mut self) -> Result<Vec<SpeechResult>, RecognizerError> {
        // The diarizer labels its tail first, so the last words get a speaker.
        if let Some(diarizer) = self.diarizer.as_mut()
            && diarizer.finish().is_err()
        {
            self.diarizer = None;
        }
        // SAFETY: `stream` is open; finishing only flushes buffered audio.
        if unsafe { (self.model.api.finish)(self.stream) } != STATUS_OK {
            return Err(RecognizerError::RecognitionFailed);
        }
        self.drain()
    }
}

impl Drop for NativeNemotron {
    fn drop(&mut self) {
        // SAFETY: the stream was created by this library and is closed once; the
        // shared model (and the library) outlives it through the `Arc`.
        unsafe { (self.model.api.close)(self.stream) };
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
        assert_eq!(size_of::<StreamingConfig>(), 24);
        assert_eq!(size_of::<RecognizerConfig>(), 88);
        assert_eq!(size_of::<RecognitionOptions>(), 72);
    }

    #[test]
    fn missing_library_or_model_is_reported_without_loading_anything() {
        let empty = std::env::temp_dir().join("anycaptions-missing-nemo");
        let paths = NemotronPaths {
            library_dir: empty.clone(),
            model: empty.join("missing.gguf"),
            diarization: empty.join("missing-diarization.gguf"),
        };
        assert!(matches!(
            NativeNemotron::load(
                &paths,
                None,
                crate::transcription::tuning::TRANSCRIPTION,
                false
            ),
            Err(RecognizerError::ModelUnavailable)
        ));
        assert!(matches!(
            Api::load(&empty),
            Err(RecognizerError::LibraryUnavailable)
        ));
    }

    #[test]
    fn a_missing_diarization_model_is_told_apart_from_a_missing_recognizer() {
        // The ASR model check runs first; with it present but the diarizer
        // missing, the error names the diarizer so the caller can fall back.
        let dir =
            std::env::temp_dir().join(format!("anycaptions-diar-missing-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fake_asr = dir.join("asr.gguf");
        std::fs::write(&fake_asr, b"not a model").unwrap();
        let paths = NemotronPaths {
            library_dir: dir.clone(),
            model: fake_asr,
            diarization: dir.join("missing-diarization.gguf"),
        };
        // A corrupted ASR file still fails as the recognizer, never as the diarizer.
        assert!(matches!(
            NemotronModel::load(&paths, None, true),
            Err(RecognizerError::ModelUnavailable)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
