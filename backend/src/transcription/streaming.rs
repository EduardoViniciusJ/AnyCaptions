use crate::{
    domain::CaptionMode,
    ipc::{CaptionUpdate, SpeakerSpan},
    pipeline::{AudioProcessor, Cancellation, Message, PipelineError},
};

/// Longest caption text forwarded to the UI; longer engine output is treated as a failure.
pub const MAX_CAPTION_CHARS: usize = 4_000;

/// Interim results: a speaker change on the newest words is shown only once at
/// least this many words belong to the new speaker. The diarizer labels the
/// freshest audio provisionally, so a one-word flip would make the label
/// flicker; the final result is exact and ignores this.
const INTERIM_MIN_TURN_WORDS: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecognizerError {
    LibraryUnavailable,
    ModelUnavailable,
    /// The diarization model is missing, corrupted or failed to load; the
    /// recognizer itself may still work without speaker labels.
    DiarizationUnavailable,
    InvalidAudio,
    RecognitionFailed,
}

/// One recognized word with the diarizer's speaker slot: 1-based, generic
/// ("Person 1"); `0` means the diarizer has not labeled it (yet).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedWord {
    pub text: String,
    pub speaker: u32,
}

/// One recognition result: the whole utterance so far, or its final text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechResult {
    pub text: String,
    pub is_final: bool,
    /// Language detected by the model (BCP-47); only final results carry it.
    pub language: Option<String>,
    /// Per-word speaker tags, in transcript order; empty when the stream runs
    /// without diarization.
    pub words: Vec<TaggedWord>,
}

/// Groups the caption's words by speaker, in order. The word-level tags are
/// only trusted when they line up one-to-one with the caption's whitespace
/// words (the library's words are the transcript's words for Latin-script
/// languages; scripts timed per character, such as Chinese, do not line up).
/// Otherwise the whole caption gets its dominant speaker, which is still a
/// true statement about the utterance without pretending word precision.
/// Returns no spans when there are no tags at all (diarization off).
pub(crate) fn speaker_spans(text: &str, words: &[TaggedWord], is_final: bool) -> Vec<SpeakerSpan> {
    if words.is_empty() {
        return Vec::new();
    }
    let caption_words: Vec<&str> = text.split_whitespace().collect();
    if caption_words.is_empty() {
        return Vec::new();
    }
    let mut tags: Vec<u32> = if caption_words.len() == words.len() {
        words.iter().map(|word| word.speaker).collect()
    } else {
        vec![dominant_speaker(words); caption_words.len()]
    };
    // Words the diarizer has not covered continue the current speaker; only a
    // run of untagged words at the very start stays unlabeled.
    for index in 1..tags.len() {
        if tags[index] == 0 {
            tags[index] = tags[index - 1];
        }
    }
    let mut runs: Vec<(u32, usize)> = Vec::new();
    for tag in tags {
        match runs.last_mut() {
            Some((speaker, count)) if *speaker == tag => *count += 1,
            _ => runs.push((tag, 1)),
        }
    }
    if !is_final
        && runs.len() > 1
        && runs
            .last()
            .is_some_and(|(_, count)| *count < INTERIM_MIN_TURN_WORDS)
    {
        let (_, count) = runs.pop().unwrap_or_default();
        if let Some((_, previous)) = runs.last_mut() {
            *previous += count;
        }
    }
    let mut spans = Vec::with_capacity(runs.len());
    let mut start = 0;
    for (speaker, count) in runs {
        spans.push(SpeakerSpan {
            speaker,
            text: caption_words[start..start + count].join(" "),
        });
        start += count;
    }
    spans
}

/// The speaker with the most tagged words (earliest wins a tie); `0` when no
/// word is tagged.
fn dominant_speaker(words: &[TaggedWord]) -> u32 {
    let mut counts: Vec<(u32, usize)> = Vec::new();
    for word in words.iter().filter(|word| word.speaker > 0) {
        match counts
            .iter_mut()
            .find(|(speaker, _)| *speaker == word.speaker)
        {
            Some((_, count)) => *count += 1,
            None => counts.push((word.speaker, 1)),
        }
    }
    // `counts` is in order of first appearance; on a tie the earliest wins
    // (`max_by_key` alone would pick the last equal element).
    counts
        .iter()
        .enumerate()
        .max_by_key(|(index, (_, count))| (*count, std::cmp::Reverse(*index)))
        .map_or(0, |(_, (speaker, _))| *speaker)
}

/// A streaming speech recognizer fed with 16 kHz mono audio. It detects speech and the end of
/// each utterance itself, so no external VAD or segmentation is needed.
#[cfg_attr(test, mockall::automock)]
pub trait StreamingRecognizer: Send {
    fn push(&mut self, samples: &[f32]) -> Result<Vec<SpeechResult>, RecognizerError>;
    /// Flushes buffered audio when capture stops.
    fn finish(&mut self) -> Result<Vec<SpeechResult>, RecognizerError>;
}

/// Turns recognizer results into caption updates: each utterance is a segment, and every new
/// text for it is a higher revision.
pub struct StreamingProcessor<R: StreamingRecognizer> {
    session_id: u32,
    mode: CaptionMode,
    recognizer: R,
    segment_id: u32,
    revision: u32,
    last_text: String,
    last_speakers: Vec<SpeakerSpan>,
}

impl<R: StreamingRecognizer> StreamingProcessor<R> {
    pub fn new(session_id: u32, mode: CaptionMode, recognizer: R) -> Self {
        Self {
            session_id,
            mode,
            recognizer,
            segment_id: 1,
            revision: 0,
            last_text: String::new(),
            last_speakers: Vec::new(),
        }
    }

    fn updates(
        &mut self,
        results: Vec<SpeechResult>,
    ) -> Result<Option<Vec<CaptionUpdate>>, PipelineError> {
        let mut updates = Vec::new();
        for result in results {
            let text = result.text.trim();
            if text.len() > MAX_CAPTION_CHARS {
                return Err(PipelineError::ProcessorFailed);
            }
            let speakers = speaker_spans(text, &result.words, result.is_final);
            // Interim results repeat while no new word is decoded (and no
            // speaker tag settles); a final always counts.
            let unchanged =
                !result.is_final && text == self.last_text && speakers == self.last_speakers;
            if !text.is_empty() && !unchanged {
                self.revision = self
                    .revision
                    .checked_add(1)
                    .ok_or(PipelineError::CounterExhausted)?;
                updates.push(CaptionUpdate {
                    session_id: self.session_id,
                    segment_id: self.segment_id,
                    revision: self.revision,
                    mode: self.mode,
                    text: text.to_owned(),
                    is_final: result.is_final,
                    language: result.language.clone(),
                    speakers: speakers.clone(),
                });
                self.last_text = text.to_owned();
                self.last_speakers = speakers;
            }
            if result.is_final {
                self.segment_id = self
                    .segment_id
                    .checked_add(1)
                    .ok_or(PipelineError::CounterExhausted)?;
                self.revision = 0;
                self.last_text.clear();
                self.last_speakers.clear();
            }
        }
        Ok((!updates.is_empty()).then_some(updates))
    }
}

impl<R: StreamingRecognizer + 'static> AudioProcessor for StreamingProcessor<R> {
    type Output = Vec<CaptionUpdate>;

    fn process(
        &mut self,
        audio: &Message<Box<[f32]>>,
        cancellation: &Cancellation,
    ) -> Result<Option<Self::Output>, PipelineError> {
        if cancellation.is_cancelled() {
            return Ok(None);
        }
        let results = self
            .recognizer
            .push(&audio.value)
            .map_err(|_| PipelineError::ProcessorFailed)?;
        if cancellation.is_cancelled() {
            return Ok(None);
        }
        self.updates(results)
    }

    fn finish(
        &mut self,
        cancellation: &Cancellation,
    ) -> Result<Option<Self::Output>, PipelineError> {
        if cancellation.is_cancelled() {
            return Ok(None);
        }
        let results = self
            .recognizer
            .finish()
            .map_err(|_| PipelineError::ProcessorFailed)?;
        self.updates(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(text: &str, is_final: bool) -> SpeechResult {
        SpeechResult {
            text: text.into(),
            is_final,
            language: is_final.then(|| "en-US".to_owned()),
            words: Vec::new(),
        }
    }

    fn tagged(text: &str, tags: &[u32]) -> Vec<TaggedWord> {
        text.split_whitespace()
            .zip(tags)
            .map(|(word, speaker)| TaggedWord {
                text: word.into(),
                speaker: *speaker,
            })
            .collect()
    }

    fn spans(spans: &[(u32, &str)]) -> Vec<SpeakerSpan> {
        spans
            .iter()
            .map(|(speaker, text)| SpeakerSpan {
                speaker: *speaker,
                text: (*text).to_owned(),
            })
            .collect()
    }

    #[test]
    fn captions_without_diarization_carry_no_speaker_spans() {
        assert!(speaker_spans("Hello there", &[], true).is_empty());
        assert!(speaker_spans("", &tagged("Hello", &[1]), true).is_empty());
    }

    #[test]
    fn aligned_word_tags_split_the_caption_at_speaker_turns() {
        let text = "Did you finish? Yes, just now.";
        let words = tagged(text, &[1, 1, 1, 2, 2, 2]);
        assert_eq!(
            speaker_spans(text, &words, true),
            spans(&[(1, "Did you finish?"), (2, "Yes, just now.")])
        );
        // A single speaker is one span over the whole caption.
        let words = tagged(text, &[2, 2, 2, 2, 2, 2]);
        assert_eq!(
            speaker_spans(text, &words, true),
            spans(&[(2, "Did you finish? Yes, just now.")])
        );
    }

    #[test]
    fn untagged_words_stay_unlabeled_only_at_the_start() {
        let text = "well so what now";
        // Nothing labeled yet: one unknown span, no label asserted.
        assert_eq!(
            speaker_spans(text, &tagged(text, &[0, 0, 0, 0]), false),
            spans(&[(0, "well so what now")])
        );
        // Leading untagged words stay unknown; later gaps continue the speaker.
        assert_eq!(
            speaker_spans(text, &tagged(text, &[0, 1, 0, 1]), true),
            spans(&[(0, "well"), (1, "so what now")])
        );
    }

    #[test]
    fn interim_results_hold_a_one_word_speaker_flip_until_it_settles() {
        let text = "I think we should go";
        let words = tagged(text, &[1, 1, 1, 1, 2]);
        // Interim: the newest word's new speaker is provisional, keep speaker 1.
        assert_eq!(
            speaker_spans(text, &words, false),
            spans(&[(1, "I think we should go")])
        );
        // Two words from the new speaker are shown as a turn.
        let text = "I think we should go now";
        let words = tagged(text, &[1, 1, 1, 1, 2, 2]);
        assert_eq!(
            speaker_spans(text, &words, false),
            spans(&[(1, "I think we should"), (2, "go now")])
        );
        // The final is exact, even for a one-word turn.
        let text = "I think we should go";
        let words = tagged(text, &[1, 1, 1, 1, 2]);
        assert_eq!(
            speaker_spans(text, &words, true),
            spans(&[(1, "I think we should"), (2, "go")])
        );
    }

    #[test]
    fn misaligned_tags_fall_back_to_the_dominant_speaker() {
        // Per-character timing (CJK) yields more tags than whitespace words.
        let words = tagged("a b c d e", &[2, 2, 1, 2, 0]);
        assert_eq!(
            speaker_spans("今日は いい天気", &words, true),
            spans(&[(2, "今日は いい天気")])
        );
        // No tagged word at all: unknown speaker, no label.
        let words = tagged("a b", &[0, 0]);
        assert_eq!(
            speaker_spans("one two three", &words, true),
            spans(&[(0, "one two three")])
        );
        // Ties go to the earliest speaker.
        let words = tagged("a b c", &[3, 1, 1]);
        assert_eq!(dominant_speaker(&words), 1);
        let words = tagged("a b c d", &[3, 1, 3, 1]);
        assert_eq!(dominant_speaker(&words), 3);
    }

    #[test]
    fn a_settled_speaker_tag_revises_an_otherwise_unchanged_interim() {
        let mut recognizer = MockStreamingRecognizer::new();
        let mut batches = vec![
            vec![SpeechResult {
                text: "Yes we did".into(),
                is_final: false,
                language: None,
                words: tagged("Yes we did", &[0, 0, 0]),
            }],
            // Same text: the diarizer caught up and labeled it.
            vec![SpeechResult {
                text: "Yes we did".into(),
                is_final: false,
                language: None,
                words: tagged("Yes we did", &[1, 1, 1]),
            }],
            // Same text and tags: nothing new to send.
            vec![SpeechResult {
                text: "Yes we did".into(),
                is_final: false,
                language: None,
                words: tagged("Yes we did", &[1, 1, 1]),
            }],
            vec![SpeechResult {
                text: "Yes we did. No!".into(),
                is_final: true,
                language: Some("en-US".into()),
                words: tagged("Yes we did. No!", &[1, 1, 1, 2]),
            }],
        ]
        .into_iter();
        recognizer
            .expect_push()
            .returning(move |_| Ok(batches.next().unwrap_or_default()));
        let mut processor = processor(recognizer);
        let cancel = Cancellation::default();

        let first = processor
            .process(&message(2_048), &cancel)
            .unwrap()
            .unwrap();
        assert_eq!(first[0].speakers, spans(&[(0, "Yes we did")]));
        let second = processor
            .process(&message(2_048), &cancel)
            .unwrap()
            .unwrap();
        assert_eq!(
            (second[0].revision, second[0].text.as_str()),
            (2, "Yes we did")
        );
        assert_eq!(second[0].speakers, spans(&[(1, "Yes we did")]));
        assert_eq!(processor.process(&message(2_048), &cancel).unwrap(), None);
        let last = processor
            .process(&message(2_048), &cancel)
            .unwrap()
            .unwrap();
        assert!(last[0].is_final);
        assert_eq!(last[0].speakers, spans(&[(1, "Yes we did."), (2, "No!")]));
        // The next segment starts clean.
        assert!(processor.last_speakers.is_empty());
    }

    fn message(samples: usize) -> Message<Box<[f32]>> {
        Message {
            session_id: 1,
            sequence: 1,
            discontinuity: false,
            value: vec![0.1; samples].into_boxed_slice(),
        }
    }

    fn processor(
        recognizer: MockStreamingRecognizer,
    ) -> StreamingProcessor<MockStreamingRecognizer> {
        StreamingProcessor::new(7, CaptionMode::Auto, recognizer)
    }

    #[test]
    fn interim_results_revise_one_segment_and_a_final_starts_the_next() {
        let mut recognizer = MockStreamingRecognizer::new();
        let mut batches = vec![
            vec![result("And so", false)],
            vec![result("And so", false), result("And so my fellow", false)],
            vec![result("And so my fellow Americans.", true)],
            vec![result("Ask not", false)],
        ]
        .into_iter();
        recognizer
            .expect_push()
            .returning(move |_| Ok(batches.next().unwrap_or_default()));
        let mut processor = processor(recognizer);
        let cancel = Cancellation::default();

        let first = processor
            .process(&message(2_048), &cancel)
            .unwrap()
            .unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!((first[0].segment_id, first[0].revision), (1, 1));
        assert!(!first[0].is_final);

        // A repeated interim text is not sent again.
        let second = processor
            .process(&message(2_048), &cancel)
            .unwrap()
            .unwrap();
        assert_eq!(second.len(), 1);
        assert_eq!(second[0].text, "And so my fellow");
        assert_eq!(second[0].revision, 2);

        let last = processor
            .process(&message(2_048), &cancel)
            .unwrap()
            .unwrap();
        assert!(last[0].is_final);
        assert_eq!((last[0].segment_id, last[0].revision), (1, 3));
        // The detected language reaches the UI only when the model commits the utterance.
        assert_eq!(first[0].language, None);
        assert_eq!(last[0].language.as_deref(), Some("en-US"));
        // Speaker labels off: the wire format is exactly as before.
        assert!(first[0].speakers.is_empty() && last[0].speakers.is_empty());

        let next = processor
            .process(&message(2_048), &cancel)
            .unwrap()
            .unwrap();
        assert_eq!((next[0].segment_id, next[0].revision), (2, 1));
        assert_eq!(next[0].session_id, 7);
    }

    #[test]
    fn silence_and_empty_results_produce_no_captions() {
        let mut recognizer = MockStreamingRecognizer::new();
        recognizer
            .expect_push()
            .returning(|_| Ok(vec![result("", false), result("   ", true)]));
        let mut processor = processor(recognizer);
        let cancel = Cancellation::default();
        assert_eq!(processor.process(&message(2_048), &cancel).unwrap(), None);
        // The empty final still closed its segment.
        assert_eq!(processor.segment_id, 2);
    }

    #[test]
    fn finish_flushes_the_last_utterance_and_cancellation_skips_work() {
        let mut recognizer = MockStreamingRecognizer::new();
        recognizer
            .expect_finish()
            .times(1)
            .returning(|| Ok(vec![result("Thank you.", true)]));
        recognizer.expect_push().times(0);
        let mut processor = processor(recognizer);

        let cancelled = Cancellation::default();
        cancelled.cancel();
        assert_eq!(
            processor.process(&message(2_048), &cancelled).unwrap(),
            None
        );
        assert_eq!(processor.finish(&cancelled).unwrap(), None);

        let flushed = processor.finish(&Cancellation::default()).unwrap().unwrap();
        assert_eq!(flushed[0].text, "Thank you.");
        assert!(flushed[0].is_final);
    }

    #[test]
    fn engine_failures_and_oversized_text_fail_the_processor() {
        let mut recognizer = MockStreamingRecognizer::new();
        let mut calls = 0;
        recognizer.expect_push().returning(move |_| {
            calls += 1;
            if calls == 1 {
                Err(RecognizerError::RecognitionFailed)
            } else {
                Ok(vec![result(&"a".repeat(MAX_CAPTION_CHARS + 1), false)])
            }
        });
        let mut processor = processor(recognizer);
        let cancel = Cancellation::default();
        assert_eq!(
            processor.process(&message(2_048), &cancel),
            Err(PipelineError::ProcessorFailed)
        );
        assert_eq!(
            processor.process(&message(2_048), &cancel),
            Err(PipelineError::ProcessorFailed)
        );
    }
}
