//! Standalone streaming diarizer through the NeMo-Speech.cpp C API
//! (`include/nemo_speech/diar.h`, exported by the same library as the
//! recognizer in both the v0.2.0 and v0.3.0 builds). The Sortformer stream is
//! fed the very same 16 kHz audio as the recognizer and yields per-frame
//! speaker probabilities (10 ms frames for Nemotron 3 Diarization). Words are
//! attributed here, in Rust, rather than by the recognizer's own sidecar:
//! final words by their exact times from the recognizer, interim words by an
//! estimate of when they were decoded, so the overlay can show a provisional
//! "Person N" before the utterance closes. One diarizer instance serves both.

use std::{
    ffi::{CString, c_char},
    ptr,
    sync::Arc,
};

use libloading::Library;

use super::{
    nemotron::{LIBRARY_FILENAME, NemotronPaths, load_library},
    streaming::{RecognizerError, TaggedWord},
};
use crate::models::catalog::NEMOTRON_3_DIARIZATION_Q8;

const STATUS_OK: i32 = 0;
// Audio pushed to the stream is already 16 kHz mono; 0 tells the library not to resample.
const MODEL_RATE: i32 = 0;

/// A word is attributed by the speaker activity right after its onset: late
/// punctuation can stretch a word's span into the next person's turn, so only
/// this much of it counts (the library's own anchor for word tags).
pub(crate) const WORD_ANCHOR_MS: f32 = 160.0;

/// How long before the recognizer's decode position an interim word started,
/// roughly: the encoder's right context plus the step that emits the word.
/// Measured against the exact final times on the AMI meeting excerpt.
pub(crate) const INTERIM_WORD_LAG_MS: f32 = 450.0;

/// When one interim result brings several new words at once, they are spread
/// this far apart going backwards from the decode position.
pub(crate) const INTERIM_WORD_SPACING_MS: f32 = 240.0;

// ---- C ABI mirror of diar.h. Field order and types must match the header exactly. ----

#[repr(C)]
struct DiarModelConfig {
    size: usize,
    model_path: *const c_char,
    gpu: i32, // -1 = CPU
    preset: *const c_char,
    chunk_frames: i32,
    right_context_frames: i32,
    left_context_frames: i32,
    fifo_frames: i32,
    spkcache_frames: i32,
    update_period_frames: i32,
}

#[repr(C)]
struct DiarModel {
    _opaque: [u8; 0],
}
#[repr(C)]
struct DiarStream {
    _opaque: [u8; 0],
}

type CreateFn = unsafe extern "C" fn(*const DiarModelConfig, *mut *mut DiarModel) -> i32;
type DestroyFn = unsafe extern "C" fn(*mut DiarModel);
type NumSpeakersFn = unsafe extern "C" fn(*const DiarModel) -> i32;
type SecondsPerFrameFn = unsafe extern "C" fn(*const DiarModel) -> f64;
type StreamOpenFn = unsafe extern "C" fn(*mut DiarModel, *mut *mut DiarStream) -> i32;
type PushFn = unsafe extern "C" fn(*mut DiarStream, *const f32, usize, i32) -> i32;
type FinishFn = unsafe extern "C" fn(*mut DiarStream) -> i32;
type CloseFn = unsafe extern "C" fn(*mut DiarStream);
type FrameCountFn = unsafe extern "C" fn(*const DiarStream) -> i64;
type FrameProbsStartFn = unsafe extern "C" fn(*const DiarStream) -> i64;
type FrameProbsFn = unsafe extern "C" fn(*const DiarStream, *mut f32, usize) -> i32;

/// Function pointers resolved from the library; the library stays loaded for
/// the whole process (see `nemotron::load_library`).
struct Api {
    create: CreateFn,
    destroy: DestroyFn,
    num_speakers: NumSpeakersFn,
    seconds_per_frame: SecondsPerFrameFn,
    stream_open: StreamOpenFn,
    push: PushFn,
    finish: FinishFn,
    close: CloseFn,
    frame_count: FrameCountFn,
    frame_probs_start: FrameProbsStartFn,
    frame_probs: FrameProbsFn,
    _library: &'static Library,
}

impl Api {
    fn load(library_dir: &std::path::Path) -> Result<Self, RecognizerError> {
        let library = load_library(&library_dir.join(LIBRARY_FILENAME))?;
        // SAFETY: the names and signatures below follow diar.h of the pinned NeMo-Speech.cpp
        // release; the library is never unloaded, so the pointers stay valid.
        unsafe {
            macro_rules! symbol {
                ($name:literal) => {
                    *library
                        .get($name)
                        .map_err(|_| RecognizerError::DiarizationUnavailable)?
                };
            }
            Ok(Self {
                create: symbol!(b"nemo_speech_diar_create\0"),
                destroy: symbol!(b"nemo_speech_diar_destroy\0"),
                num_speakers: symbol!(b"nemo_speech_diar_num_speakers\0"),
                seconds_per_frame: symbol!(b"nemo_speech_diar_seconds_per_frame\0"),
                stream_open: symbol!(b"nemo_speech_diar_stream_open\0"),
                push: symbol!(b"nemo_speech_diar_stream_push_f32\0"),
                finish: symbol!(b"nemo_speech_diar_stream_finish\0"),
                close: symbol!(b"nemo_speech_diar_stream_close\0"),
                frame_count: symbol!(b"nemo_speech_diar_frame_count\0"),
                frame_probs_start: symbol!(b"nemo_speech_diar_frame_probs_start\0"),
                frame_probs: symbol!(b"nemo_speech_diar_frame_probs\0"),
                _library: library,
            })
        }
    }
}

/// The loaded Sortformer model. Like the recognizer, loaded once and shared.
pub struct DiarizerModel {
    model: *mut DiarModel,
    api: Api,
    speakers: usize,
    seconds_per_frame: f64,
}

// SAFETY: diar.h binds neither the model nor its streams to a thread; streams
// are single-threaded by contract and each is owned by one recognizer stream.
unsafe impl Send for DiarizerModel {}
unsafe impl Sync for DiarizerModel {}

impl DiarizerModel {
    /// Loads the diarization GGUF (verified against the catalog) on the GPU,
    /// falling back to the CPU; keeps the library's model-specific streaming
    /// geometry (Nemotron 3 Diarization: 13-frame chunks, 1 frame of right
    /// context, about 1.1 s of attribution latency).
    pub fn load(paths: &NemotronPaths) -> Result<Arc<Self>, RecognizerError> {
        NEMOTRON_3_DIARIZATION_Q8
            .verify(&paths.diarization)
            .map_err(|_| RecognizerError::DiarizationUnavailable)?;
        let model_path = paths
            .diarization
            .to_str()
            .and_then(|path| CString::new(path).ok())
            .ok_or(RecognizerError::DiarizationUnavailable)?;
        let api = Api::load(&paths.library_dir)?;
        let mut model = ptr::null_mut();
        for gpu in [0, -1] {
            let config = DiarModelConfig {
                size: size_of::<DiarModelConfig>(),
                model_path: model_path.as_ptr(),
                gpu,
                preset: ptr::null(),
                chunk_frames: 0,
                right_context_frames: 0,
                left_context_frames: -1,
                fifo_frames: 0,
                spkcache_frames: 0,
                update_period_frames: 0,
            };
            // SAFETY: the config and its strings live for the call; NULL/0 fields take defaults.
            let status = unsafe { (api.create)(&config, &mut model) };
            if status == STATUS_OK && !model.is_null() {
                if gpu < 0 {
                    eprintln!("[diar] gpu unavailable, diarizer loaded on cpu");
                }
                break;
            }
            model = ptr::null_mut();
        }
        if model.is_null() {
            return Err(RecognizerError::DiarizationUnavailable);
        }
        // SAFETY: `model` was just created and is valid until destroyed.
        let (speakers, seconds_per_frame) =
            unsafe { ((api.num_speakers)(model), (api.seconds_per_frame)(model)) };
        if speakers <= 0 || !seconds_per_frame.is_finite() || seconds_per_frame <= 0.0 {
            // SAFETY: created above; nothing else references it.
            unsafe { (api.destroy)(model) };
            return Err(RecognizerError::DiarizationUnavailable);
        }
        Ok(Arc::new(Self {
            model,
            api,
            speakers: speakers as usize,
            seconds_per_frame,
        }))
    }

    /// Speaker slots the model can tell apart (8 for Nemotron 3 Diarization).
    pub fn speakers(&self) -> usize {
        self.speakers
    }
}

impl Drop for DiarizerModel {
    fn drop(&mut self) {
        // SAFETY: every stream keeps an `Arc` to this model, so all streams are
        // closed before the model is destroyed.
        unsafe { (self.api.destroy)(self.model) };
    }
}

/// The per-frame speaker probabilities emitted so far, as copied out of the
/// library: frame-major, `speakers` floats per frame, covering frames
/// `[base, base + probs.len() / speakers)`.
#[derive(Debug, Default, Clone)]
pub(crate) struct Timeline {
    pub probs: Vec<f32>,
    pub speakers: usize,
    pub base: i64,
    pub ms_per_frame: f32,
}

impl Timeline {
    fn frame_count(&self) -> i64 {
        if self.speakers == 0 {
            return 0;
        }
        self.base + (self.probs.len() / self.speakers) as i64
    }

    /// 1-based speaker whose activity dominates frames `[start, end)`, or 0
    /// before anything was emitted. Ranges past the emitted frontier use the
    /// last frame (the library's own extrapolation for fresh words), ranges
    /// before the retained window its first frame.
    pub(crate) fn speaker_for_frames(&self, start: i64, end: i64) -> u32 {
        let count = self.frame_count();
        if count == 0 || self.probs.is_empty() {
            return 0;
        }
        let start = start.max(self.base).min(count - 1);
        let end = end.min(count).max(start + 1);
        let mut mean = vec![0.0_f32; self.speakers];
        for frame in start..end {
            let offset = ((frame - self.base) as usize) * self.speakers;
            for (speaker, value) in mean.iter_mut().enumerate() {
                *value += self.probs[offset + speaker];
            }
        }
        let best = mean.iter().enumerate().fold(
            0,
            |best, (index, value)| {
                if *value > mean[best] { index } else { best }
            },
        );
        best as u32 + 1
    }

    /// Speaker for a word spoken over `[start_ms, end_ms)`, anchored on its onset.
    pub(crate) fn speaker_for_word(&self, start_ms: f32, end_ms: f32) -> u32 {
        if self.ms_per_frame <= 0.0 {
            return 0;
        }
        let anchor_end = end_ms.min(start_ms + WORD_ANCHOR_MS);
        let start = (start_ms / self.ms_per_frame).floor() as i64;
        let end = (anchor_end / self.ms_per_frame).ceil() as i64;
        self.speaker_for_frames(start, end)
    }
}

/// One diarization stream, fed the recognizer's audio in lockstep.
pub struct DiarizerStream {
    stream: *mut DiarStream,
    model: Arc<DiarizerModel>,
    timeline: Timeline,
}

// SAFETY: owned by one recognizer stream and used only through `&mut self`.
unsafe impl Send for DiarizerStream {}

impl DiarizerStream {
    pub fn open(model: Arc<DiarizerModel>) -> Result<Self, RecognizerError> {
        let mut stream = ptr::null_mut();
        // SAFETY: the model outlives the stream through the `Arc`.
        let status = unsafe { (model.api.stream_open)(model.model, &mut stream) };
        if status != STATUS_OK || stream.is_null() {
            return Err(RecognizerError::DiarizationUnavailable);
        }
        let timeline = Timeline {
            probs: Vec::new(),
            speakers: model.speakers,
            base: 0,
            ms_per_frame: (model.seconds_per_frame * 1000.0) as f32,
        };
        Ok(Self {
            stream,
            model,
            timeline,
        })
    }

    pub fn push(&mut self, samples: &[f32]) -> Result<(), RecognizerError> {
        // SAFETY: `stream` is open and the slice is valid for `samples.len()` floats.
        let status = unsafe {
            (self.model.api.push)(self.stream, samples.as_ptr(), samples.len(), MODEL_RATE)
        };
        if status != STATUS_OK {
            return Err(RecognizerError::DiarizationUnavailable);
        }
        self.refresh()
    }

    /// No more audio: labels the remaining tail so the last words get a speaker.
    pub fn finish(&mut self) -> Result<(), RecognizerError> {
        // SAFETY: `stream` is open; finishing only flushes buffered audio.
        if unsafe { (self.model.api.finish)(self.stream) } != STATUS_OK {
            return Err(RecognizerError::DiarizationUnavailable);
        }
        self.refresh()
    }

    /// Copies out the retained probabilities when the diarizer advanced (it
    /// labels whole chunks, about once per second, so most pushes copy nothing).
    fn refresh(&mut self) -> Result<(), RecognizerError> {
        // SAFETY: `stream` is open; the counts describe the buffer the next call fills.
        let (count, base) = unsafe {
            (
                (self.model.api.frame_count)(self.stream),
                (self.model.api.frame_probs_start)(self.stream),
            )
        };
        if count <= self.timeline.frame_count() || count <= base {
            return Ok(());
        }
        let floats = ((count - base) as usize) * self.model.speakers;
        let mut probs = vec![0.0_f32; floats];
        // SAFETY: `probs` holds exactly the capacity passed; the library writes at most that.
        let status =
            unsafe { (self.model.api.frame_probs)(self.stream, probs.as_mut_ptr(), floats) };
        if status != STATUS_OK {
            return Err(RecognizerError::DiarizationUnavailable);
        }
        self.timeline.probs = probs;
        self.timeline.base = base;
        Ok(())
    }

    pub(crate) fn timeline(&self) -> &Timeline {
        &self.timeline
    }
}

impl Drop for DiarizerStream {
    fn drop(&mut self) {
        // SAFETY: the stream was opened by this library and is closed once.
        unsafe { (self.model.api.close)(self.stream) };
    }
}

/// The words of the utterance being decoded, with the estimated onset of each
/// (ms since the stream started). Interim results carry no word times, so new
/// words are placed just behind the recognizer's decode position and
/// re-attributed on every update as the diarizer catches up; the final result
/// replaces all of this with exact times.
#[derive(Debug, Default)]
pub(crate) struct InterimWords {
    words: Vec<(String, f32)>,
}

impl InterimWords {
    /// Folds in an interim transcript decoded up to `decoded_ms`: words that
    /// match the known prefix keep their estimate, the rest are (re)placed.
    pub(crate) fn update(&mut self, text: &str, decoded_ms: f32) {
        let incoming: Vec<&str> = text.split_whitespace().collect();
        let kept = self
            .words
            .iter()
            .zip(&incoming)
            .take_while(|((known, _), word)| known == *word)
            .count();
        self.words.truncate(kept);
        let fresh = incoming.len() - kept;
        for (index, word) in incoming[kept..].iter().enumerate() {
            let behind = (fresh - 1 - index) as f32;
            let onset =
                (decoded_ms - INTERIM_WORD_LAG_MS - behind * INTERIM_WORD_SPACING_MS).max(0.0);
            self.words.push(((*word).to_owned(), onset));
        }
    }

    pub(crate) fn clear(&mut self) {
        self.words.clear();
    }

    /// Attributes every word on the current timeline.
    pub(crate) fn tagged(&self, timeline: &Timeline) -> Vec<TaggedWord> {
        self.words
            .iter()
            .map(|(text, onset)| TaggedWord {
                text: text.clone(),
                speaker: timeline.speaker_for_word(*onset, *onset + WORD_ANCHOR_MS),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timeline(frames: &[[f32; 2]], ms_per_frame: f32) -> Timeline {
        Timeline {
            probs: frames.iter().flatten().copied().collect(),
            speakers: 2,
            base: 0,
            ms_per_frame,
        }
    }

    #[test]
    fn config_struct_matches_the_c_layout() {
        // diar.h on x64: size_t + pointer + int32 (padded) + pointer + 6 × int32.
        assert_eq!(size_of::<DiarModelConfig>(), 56);
    }

    #[test]
    fn words_take_the_speaker_dominating_their_onset() {
        // 10 ms frames: speaker 1 for 0–500 ms, speaker 2 afterwards.
        let mut frames = vec![[0.9, 0.1]; 50];
        frames.extend(vec![[0.2, 0.8]; 50]);
        let timeline = timeline(&frames, 10.0);
        assert_eq!(timeline.speaker_for_word(100.0, 300.0), 1);
        assert_eq!(timeline.speaker_for_word(600.0, 900.0), 2);
        // Onset-anchored: a word starting in speaker 1's turn that stretches far
        // into speaker 2's turn (late punctuation) still belongs to speaker 1.
        assert_eq!(timeline.speaker_for_word(400.0, 1_000.0), 1);
        // Past the frontier the last frame decides (fresh words); nothing emitted = unknown.
        assert_eq!(timeline.speaker_for_word(5_000.0, 5_200.0), 2);
        assert_eq!(Timeline::default().speaker_for_word(0.0, 100.0), 0);
        assert_eq!(
            timeline.speaker_for_frames(10, 10),
            1,
            "an empty range still counts one frame"
        );
    }

    #[test]
    fn a_retained_window_clamps_older_ranges_to_its_first_frame() {
        let mut timeline = timeline(&[[0.1, 0.9]; 20], 10.0);
        timeline.base = 1_000; // frames before 1_000 were compacted away
        assert_eq!(timeline.speaker_for_frames(0, 5), 2);
        assert_eq!(timeline.speaker_for_frames(1_005, 1_010), 2);
    }

    #[test]
    fn interim_words_keep_their_estimate_while_the_prefix_holds() {
        let mut interim = InterimWords::default();
        interim.update("did you", 1_000.0);
        let first = interim.words.clone();
        assert_eq!(first.len(), 2);
        // Two new words: the newest sits one lag behind the decode position,
        // the one before it one spacing earlier.
        assert_eq!(first[1].1, 1_000.0 - INTERIM_WORD_LAG_MS);
        assert_eq!(
            first[0].1,
            1_000.0 - INTERIM_WORD_LAG_MS - INTERIM_WORD_SPACING_MS
        );
        interim.update("did you finish", 1_300.0);
        assert_eq!(interim.words[..2], first[..]);
        assert_eq!(interim.words[2].1, 1_300.0 - INTERIM_WORD_LAG_MS);
        // A corrected prefix drops the stale words and re-estimates the new ones.
        interim.update("did we finish it", 1_600.0);
        assert_eq!(interim.words[0], first[0]);
        assert_eq!(interim.words.len(), 4);
        assert!(interim.words[1].1 < interim.words[2].1 && interim.words[2].1 < interim.words[3].1);
        // Estimates never go negative at the very start of a stream.
        let mut early = InterimWords::default();
        early.update("hi there", 100.0);
        assert!(early.words.iter().all(|(_, onset)| *onset >= 0.0));
        early.clear();
        assert!(early.words.is_empty());
    }

    #[test]
    fn interim_words_are_tagged_on_the_current_timeline() {
        let mut frames = vec![[0.9, 0.1]; 100]; // speaker 1 for the first second
        frames.extend(vec![[0.1, 0.9]; 100]); // speaker 2 for the next
        let timeline = timeline(&frames, 10.0);
        let mut interim = InterimWords::default();
        interim.update("yes", 500.0 + INTERIM_WORD_LAG_MS);
        interim.update("yes no", 1_500.0 + INTERIM_WORD_LAG_MS);
        let tags: Vec<u32> = interim
            .tagged(&timeline)
            .iter()
            .map(|word| word.speaker)
            .collect();
        assert_eq!(tags, vec![1, 2]);
        assert!(
            interim
                .tagged(&Timeline::default())
                .iter()
                .all(|word| word.speaker == 0)
        );
    }
}
