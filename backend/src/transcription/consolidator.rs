use crate::{domain::CaptionMode, ipc::CaptionUpdate};

const MAX_OVERLAP_WORDS: usize = 8;

/// Consolidates partial and final English transcriptions for a single session.
///
/// Boundary deduplication only inspects the prefix of a segment that was produced
/// with audio overlap (`has_overlap = true`) against the suffix of the immediately
/// preceding finalized segment. Legitimate repeated words inside a phrase are never removed.
pub struct CaptionConsolidator {
    session_id: u32,
    mode: CaptionMode,
    active_segment_id: u32,
    last_revision: u32,
    last_partial_text: String,
    previous_tail_words: Vec<String>,
}

impl CaptionConsolidator {
    pub fn new(session_id: u32, mode: CaptionMode) -> Self {
        Self {
            session_id,
            mode,
            active_segment_id: 0,
            last_revision: 0,
            last_partial_text: String::new(),
            previous_tail_words: Vec::new(),
        }
    }

    pub fn session_id(&self) -> u32 {
        self.session_id
    }

    pub fn reset(&mut self, session_id: u32, mode: CaptionMode) {
        self.session_id = session_id;
        self.mode = mode;
        self.active_segment_id = 0;
        self.last_revision = 0;
        self.last_partial_text.clear();
        self.previous_tail_words.clear();
    }

    pub fn push(
        &mut self,
        session_id: u32,
        segment_id: u32,
        revision: u32,
        raw_text: &str,
        has_overlap: bool,
        is_final: bool,
    ) -> Option<CaptionUpdate> {
        if session_id != self.session_id || segment_id == 0 || segment_id < self.active_segment_id {
            return None;
        }
        if segment_id == self.active_segment_id && revision <= self.last_revision {
            return None;
        }
        let cleaned = sanitize_whisper_text(raw_text);
        let deduplicated = if has_overlap && !self.previous_tail_words.is_empty() {
            strip_overlap_prefix(&cleaned, &self.previous_tail_words)
        } else {
            cleaned
        };
        if deduplicated.is_empty() {
            return None;
        }

        if segment_id > self.active_segment_id {
            self.active_segment_id = segment_id;
            self.last_partial_text.clear();
        }
        self.last_revision = revision;

        if !is_final {
            if deduplicated == self.last_partial_text {
                return None;
            }
            self.last_partial_text.clone_from(&deduplicated);
        } else {
            self.last_partial_text.clear();
            let words: Vec<String> = deduplicated
                .split_whitespace()
                .map(normalize_word)
                .filter(|w| !w.is_empty())
                .collect();
            let start = words.len().saturating_sub(MAX_OVERLAP_WORDS);
            self.previous_tail_words = words[start..].to_vec();
            self.active_segment_id = segment_id.saturating_add(1);
            self.last_revision = 0;
        }

        Some(CaptionUpdate {
            session_id: self.session_id,
            segment_id,
            revision,
            mode: self.mode,
            text: deduplicated,
            is_final,
        })
    }
}

pub fn sanitize_whisper_text(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut tokens = Vec::new();
    let mut in_brackets = false;
    let mut in_parens = false;
    let mut current = String::new();
    for ch in trimmed.chars() {
        match ch {
            '[' => {
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_owned());
                }
                current.clear();
                in_brackets = true;
            }
            ']' if in_brackets => {
                in_brackets = false;
                current.clear();
            }
            '(' => {
                if !current.trim().is_empty() {
                    tokens.push(current.trim().to_owned());
                }
                current.clear();
                in_parens = true;
            }
            ')' if in_parens => {
                in_parens = false;
                current.clear();
            }
            _ if !in_brackets && !in_parens => current.push(ch),
            _ => {}
        }
    }
    if !in_brackets && !in_parens && !current.trim().is_empty() {
        tokens.push(current.trim().to_owned());
    }
    let joined = tokens
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    collapse_repeated_clauses(&joined)
}

/// Collapses Whisper decoder loops where a clause of 3+ words is repeated consecutively
/// (e.g. "Get the spread on there. Get the spread on there."), while preserving
/// legitimate 1-2 word repetitions such as "Wait, wait! Go, go now!".
fn collapse_repeated_clauses(text: &str) -> String {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() < 6 {
        return text.to_owned();
    }
    let normalized: Vec<String> = words.iter().map(|w| normalize_word(w)).collect();
    let mut out: Vec<&str> = Vec::with_capacity(words.len());
    let mut out_norm: Vec<String> = Vec::with_capacity(words.len());

    let mut i = 0;
    while i < words.len() {
        let max_span = (words.len() - i).min(out_norm.len()).min(16);
        let mut matched_span = 0;
        for span in (3..=max_span).rev() {
            if out_norm[out_norm.len() - span..] == normalized[i..i + span] {
                matched_span = span;
                break;
            }
        }
        if matched_span >= 3 {
            i += matched_span;
        } else {
            out.push(words[i]);
            out_norm.push(normalized[i].clone());
            i += 1;
        }
    }
    // Also strip a trailing incomplete prefix of a just-repeated clause (e.g. "... on there. Get")
    if out.len() >= 5 {
        for tail_len in (1..=2).rev() {
            if out.len() > tail_len + 3 {
                let prefix_end = out.len() - tail_len;
                for span in 3..=prefix_end.min(12) {
                    let clause_start = prefix_end - span;
                    if out_norm[clause_start..clause_start + tail_len] == out_norm[prefix_end..]
                        && words.len() > out.len()
                    {
                        out.truncate(prefix_end);
                        break;
                    }
                }
            }
        }
    }
    out.join(" ")
}

fn normalize_word(word: &str) -> String {
    word.chars()
        .filter(|c| c.is_alphanumeric() || *c == '\'')
        .flat_map(char::to_lowercase)
        .collect()
}

fn strip_overlap_prefix(text: &str, previous_tail: &[String]) -> String {
    let original_words: Vec<&str> = text.split_whitespace().collect();
    let normalized_words: Vec<String> = original_words.iter().map(|w| normalize_word(w)).collect();
    let max_k = previous_tail
        .len()
        .min(normalized_words.len())
        .min(MAX_OVERLAP_WORDS);
    for k in (1..=max_k).rev() {
        let tail_slice = &previous_tail[previous_tail.len() - k..];
        let head_slice = &normalized_words[..k];
        if tail_slice == head_slice && tail_slice.iter().any(|w| !w.is_empty()) {
            return original_words[k..].join(" ");
        }
    }
    original_words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_legitimate_repeated_words_within_a_segment() {
        let mut consolidator = CaptionConsolidator::new(1, CaptionMode::EnEn);
        let update = consolidator
            .push(1, 1, 1, "Wait, wait! Go, go now!", false, true)
            .unwrap();
        assert_eq!(update.text, "Wait, wait! Go, go now!");
    }

    #[test]
    fn collapses_whisper_decoder_clause_loops() {
        let mut consolidator = CaptionConsolidator::new(1, CaptionMode::EnEn);
        let looped = "get the spread of reduction on there. Get the spread of reduction on there. Get the spread of reduction on there. Get";
        let update = consolidator.push(1, 1, 1, looped, false, true).unwrap();
        assert_eq!(update.text, "get the spread of reduction on there.");
    }

    #[test]
    fn deduplicates_boundary_words_only_when_audio_overlap_is_present() {
        let mut consolidator = CaptionConsolidator::new(1, CaptionMode::EnEn);
        consolidator
            .push(1, 1, 1, "We choose to go to the moon", false, true)
            .unwrap();
        // Segment 2 has audio overlap from segment 1 ("to the moon").
        let overlapped = consolidator
            .push(1, 2, 1, "to the moon in this decade", true, true)
            .unwrap();
        assert_eq!(overlapped.text, "in this decade");

        // Segment 3 has NO audio overlap, so a repeated phrase is kept intact.
        let intentional_repeat = consolidator
            .push(1, 3, 1, "in this decade we will succeed", false, true)
            .unwrap();
        assert_eq!(intentional_repeat.text, "in this decade we will succeed");
    }

    #[test]
    fn updates_partials_rejects_stale_revisions_and_strips_non_speech_markers() {
        let mut consolidator = CaptionConsolidator::new(5, CaptionMode::EnEn);
        assert!(
            consolidator
                .push(5, 1, 1, "[BLANK_AUDIO] (music)", false, false)
                .is_none()
        );
        let p1 = consolidator.push(5, 1, 2, "Hello", false, false).unwrap();
        assert_eq!(p1.text, "Hello");
        // Identical partial text does not emit a redundant event.
        assert!(consolidator.push(5, 1, 3, "Hello", false, false).is_none());
        // Stale revision or wrong session is rejected.
        assert!(
            consolidator
                .push(5, 1, 2, "Hello world", false, false)
                .is_none()
        );
        assert!(
            consolidator
                .push(4, 1, 4, "Hello world", false, false)
                .is_none()
        );
        let final_seg = consolidator
            .push(5, 1, 4, "Hello world!", false, true)
            .unwrap();
        assert!(final_seg.is_final);
        assert_eq!(final_seg.text, "Hello world!");
        // Late partial for an already finalized segment is rejected.
        assert!(
            consolidator
                .push(5, 1, 5, "Hello world again", false, false)
                .is_none()
        );
    }
}
