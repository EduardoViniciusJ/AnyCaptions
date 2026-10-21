#[path = "../examples/whisper_benchmark/support.rs"]
mod support;

use std::{io::Cursor, path::Path};
use support::{Artifact, Experiment, Fixture, percentile, quality, read_wav, text_sha256};

fn artifact() -> Artifact {
    Artifact {
        id: "test-fixture".into(),
        path: "fixture.wav".into(),
        sha256: "0".repeat(64),
        source: "synthetic-test".into(),
        revision: "1".into(),
        license: "test-only".into(),
    }
}

fn experiment() -> Experiment {
    Experiment {
        model: artifact(),
        fixtures: vec![Fixture {
            audio: artifact(),
            reference: "hello".into(),
        }],
        threads: 4,
        warmup_runs: 1,
        measured_runs: 5,
    }
}

struct TempDir(std::path::PathBuf);
impl TempDir {
    fn new() -> Self {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("anycaptions-whisper-{}-{id}", std::process::id()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wav(path: &Path, rate: u32, samples: usize) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut bytes, spec).unwrap();
        for i in 0..samples {
            writer
                .write_sample(if i == 0 { i16::MIN } else { 0 })
                .unwrap();
        }
        writer.finalize().unwrap();
    }
    std::fs::write(path, bytes.into_inner()).unwrap();
}

#[test]
fn word_errors_preserve_repetitions_and_count_insertions_deletions_substitutions() {
    assert_eq!(quality("Go, go now!", "go now").unwrap().word_edits, 1);
    assert_eq!(quality("go now", "go go now").unwrap().word_edits, 1);
    assert_eq!(quality("go now", "stay now").unwrap().wer, Some(0.5));
    assert_eq!(
        quality("Don't stop.", "don't STOP!").unwrap().wer,
        Some(0.0)
    );
    assert_eq!(quality("a", "b c d").unwrap().wer, Some(3.0));
    assert!(quality("", &"x".repeat(16_385)).is_err());
}

#[test]
fn silence_reports_false_positives_without_dividing_by_zero() {
    let empty = quality("", "").unwrap();
    assert_eq!(empty.wer, None);
    assert_eq!(empty.word_edits, 0);
    let hallucination = quality("", "thank you").unwrap();
    assert_eq!(hallucination.wer, None);
    assert_eq!(hallucination.hypothesis_words, 2);
}

#[test]
fn percentiles_use_nearest_rank_and_reject_invalid_samples() {
    assert_eq!(percentile(&[30.0, 10.0, 20.0, 40.0, 50.0], 50), Ok(30.0));
    assert_eq!(percentile(&[30.0, 10.0, 20.0, 40.0, 50.0], 95), Ok(50.0));
    assert!(percentile(&[], 50).is_err());
    assert!(percentile(&[f64::NAN], 95).is_err());
    assert!(percentile(&[-1.0], 50).is_err());
    assert!(percentile(&[1.0], 0).is_err());
}

#[test]
fn metadata_and_experiment_limits_are_required() {
    let mut config = experiment();
    assert!(config.validate().is_ok());
    config.warmup_runs = 0;
    assert!(config.validate().is_err());
    config.warmup_runs = 1;
    config.fixtures.push(Fixture {
        audio: artifact(),
        reference: String::new(),
    });
    assert!(config.validate().is_err());
    config.fixtures.pop();
    config.model.sha256 = "invalid".into();
    assert!(config.validate().is_err());
    config.model = artifact();
    config.model.license.clear();
    assert!(config.validate().is_err());
}

#[test]
fn checksum_rejects_corrupt_and_missing_files() {
    let dir = TempDir::new();
    let mut entry = artifact();
    assert!(entry.verify(&dir.0).is_err());
    std::fs::write(dir.0.join(&entry.path), "abc").unwrap();
    entry.sha256 = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into();
    assert!(entry.verify(&dir.0).is_ok());
    std::fs::write(dir.0.join(&entry.path), "changed").unwrap();
    assert_eq!(entry.verify(&dir.0), Err("artifact_checksum_mismatch"));
    assert_eq!(text_sha256("abc"), entry.sha256);
}

#[test]
fn wav_requires_bounded_normalized_pcm() {
    let dir = TempDir::new();
    let path = dir.0.join("fixture.wav");
    wav(&path, 16_000, 16_000);
    let samples = read_wav(&path).unwrap();
    assert_eq!(samples.len(), 16_000);
    assert_eq!(samples[0], -1.0);
    assert!(samples.iter().all(|sample| sample.is_finite()));
    wav(&path, 48_000, 48_000);
    assert_eq!(read_wav(&path), Err("expected_pcm16_mono_16000"));
    wav(&path, 16_000, 16);
    assert!(read_wav(&path).is_err());
    wav(&path, 16_000, support::MAX_SAMPLES + 1);
    assert!(read_wav(&path).is_err());
}

#[test]
fn malformed_or_oversized_config_returns_sanitized_error() {
    let dir = TempDir::new();
    let path = dir.0.join("config.json");
    std::fs::write(&path, "private invalid content").unwrap();
    assert!(matches!(
        Experiment::load(&path),
        Err("invalid_experiment_json")
    ));
    std::fs::write(&path, vec![b' '; 65_537]).unwrap();
    assert!(matches!(Experiment::load(&path), Err("input_too_large")));
}
