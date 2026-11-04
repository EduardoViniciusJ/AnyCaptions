//! Speaker labels on real multi-speaker audio: the Nemotron ASR with the
//! Nemotron 3 Diarization sidecar over a 60 s AMI meeting excerpt
//! (`benchmarks/local/ami_en2002d_2132.wav`, from NeMo-Speech.cpp's
//! `test_files/diar`, CC BY 4.0: three speakers taking turns, brief overlap,
//! a fourth with a few words). Needs the NeMo-Speech.cpp build plus both
//! GGUFs in the models root, so it is ignored by default.

use std::{collections::BTreeMap, path::PathBuf, time::Instant};

use anycaptions_lib::transcription::{
    nemotron::{NativeNemotron, NemotronModel, NemotronPaths},
    streaming::{SpeechResult, StreamingRecognizer},
    tuning::TRANSCRIPTION,
};

// Same block size the capture coordinator submits (128 ms at 16 kHz).
const BLOCK_SAMPLES: usize = 2_048;

fn load_wav_16k_mono(path: &PathBuf) -> Vec<f32> {
    let mut reader = hound::WavReader::open(path).expect("fixture wav should exist");
    let spec = reader.spec();
    assert_eq!((spec.channels, spec.sample_rate), (1, 16_000));
    reader
        .samples::<i16>()
        .map(|sample| f32::from(sample.unwrap()) / 32_768.0)
        .collect()
}

fn ami_samples() -> Vec<f32> {
    load_wav_16k_mono(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("benchmarks/local/ami_en2002d_2132.wav"),
    )
}

fn stream(recognizer: &mut NativeNemotron, samples: &[f32]) -> (Vec<SpeechResult>, u128) {
    let start = Instant::now();
    let mut results = Vec::new();
    for block in samples.chunks(BLOCK_SAMPLES) {
        results.extend(recognizer.push(block).unwrap());
    }
    results.extend(recognizer.finish().unwrap());
    (results, start.elapsed().as_millis())
}

/// Final captions as "Person N: words" turns, the way the overlay labels them.
fn labeled_turns(results: &[SpeechResult]) -> Vec<(u32, String)> {
    let mut turns: Vec<(u32, String)> = Vec::new();
    for result in results.iter().filter(|result| result.is_final) {
        for word in &result.words {
            match turns.last_mut() {
                Some((speaker, text)) if *speaker == word.speaker => {
                    text.push(' ');
                    text.push_str(&word.text);
                }
                _ => turns.push((word.speaker, word.text.clone())),
            }
        }
    }
    turns
}

#[test]
#[ignore = "requires the NeMo-Speech.cpp build, both GGUFs and the AMI fixture"]
fn diarization_labels_a_real_three_speaker_meeting() {
    let paths = NemotronPaths::resolve();
    assert!(
        paths.diarization_available(),
        "diarization GGUF missing or corrupted at {}",
        paths.diarization.display()
    );
    let audio = ami_samples();
    let audio_ms = (audio.len() / 16) as u128;

    // Plain recognizer first: the latency baseline this machine gives.
    let load = Instant::now();
    let plain_model =
        NemotronModel::load(&paths, TRANSCRIPTION.rnnt_right_context, false).expect("ASR model");
    let plain_load_ms = load.elapsed().as_millis();
    let mut plain = NativeNemotron::open(plain_model.clone(), None, TRANSCRIPTION, false)
        .expect("plain stream");
    let (plain_results, plain_decode_ms) = stream(&mut plain, &audio);
    drop(plain);
    drop(plain_model);

    // Same recognizer with the diarizer attached.
    let load = Instant::now();
    let model = NemotronModel::load(&paths, TRANSCRIPTION.rnnt_right_context, true)
        .expect("ASR + diarizer");
    let load_ms = load.elapsed().as_millis();
    assert!(model.has_diarization());
    let mut tagged =
        NativeNemotron::open(model.clone(), None, TRANSCRIPTION, true).expect("tagged stream");
    let (results, decode_ms) = stream(&mut tagged, &audio);
    eprintln!(
        "[diar] audio_ms={audio_ms} plain: load_ms={plain_load_ms} decode_ms={plain_decode_ms} | with diarizer: load_ms={load_ms} decode_ms={decode_ms}"
    );

    // Transcription itself still works and the content matches the meeting.
    let finals: Vec<&SpeechResult> = results
        .iter()
        .filter(|result| result.is_final && !result.text.trim().is_empty())
        .collect();
    let text = finals
        .iter()
        .map(|result| result.text.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        text.contains("speaker") || text.contains("organi"),
        "unexpected transcript: {text}"
    );
    let plain_text = plain_results
        .iter()
        .filter(|result| result.is_final)
        .map(|result| result.text.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(plain_results.iter().all(|result| result.words.is_empty()));
    assert!(plain_text.contains("speaker") || plain_text.contains("organi"));

    // Every final carries word tags that line up with its words, and the
    // meeting resolves to more than one speaker.
    let mut words_per_speaker: BTreeMap<u32, usize> = BTreeMap::new();
    let mut aligned = 0;
    for result in &finals {
        assert!(
            !result.words.is_empty(),
            "final without words: {}",
            result.text
        );
        if result.words.len() == result.text.split_whitespace().count() {
            aligned += 1;
        }
        for word in &result.words {
            *words_per_speaker.entry(word.speaker).or_default() += 1;
        }
    }
    eprintln!(
        "[diar] finals={} aligned_finals={aligned} words_per_speaker={words_per_speaker:?}",
        finals.len()
    );
    for (speaker, turn) in labeled_turns(&results) {
        eprintln!("[diar] Person {speaker}: {turn}");
    }
    let tagged_words: usize = words_per_speaker
        .iter()
        .filter(|(speaker, _)| **speaker > 0)
        .map(|(_, count)| count)
        .sum();
    let total_words: usize = words_per_speaker.values().sum();
    assert!(
        tagged_words * 10 >= total_words * 9,
        "most words should be tagged: {tagged_words}/{total_words}"
    );
    let speakers = words_per_speaker
        .keys()
        .filter(|speaker| **speaker > 0)
        .count();
    assert!(
        speakers >= 2,
        "expected at least two speakers: {words_per_speaker:?}"
    );
    assert!(
        aligned * 10 >= finals.len() * 8,
        "word tags should line up with the transcript words: {aligned}/{}",
        finals.len()
    );
    // Interim results are tagged provisionally (estimated word times on the
    // diarizer's timeline); the final tags, from exact word times, are the
    // reference. Report how often the last interim before each final agreed.
    let interims = results.iter().filter(|result| !result.is_final).count();
    let interims_tagged = results
        .iter()
        .filter(|result| !result.is_final && result.words.iter().any(|word| word.speaker > 0))
        .count();
    let (mut compared, mut agreed, mut finals_compared) = (0_usize, 0_usize, 0_usize);
    let mut last_interim: Option<&SpeechResult> = None;
    for result in &results {
        if !result.is_final {
            last_interim = Some(result);
            continue;
        }
        if let Some(interim) = last_interim.take()
            && interim.words.len() == result.words.len()
            && !result.words.is_empty()
        {
            finals_compared += 1;
            for (provisional, exact) in interim.words.iter().zip(&result.words) {
                compared += 1;
                if provisional.speaker == exact.speaker {
                    agreed += 1;
                }
            }
        }
    }
    eprintln!(
        "[diar] interims={interims} tagged={interims_tagged} provisional_vs_final: finals={finals_compared} words={compared} agreed={agreed}"
    );
    assert!(
        interims_tagged > 0,
        "interim results should carry provisional speaker tags"
    );
    assert!(
        compared > 0 && agreed * 10 >= compared * 7,
        "provisional tags should mostly agree with the final ones: {agreed}/{compared}"
    );

    // A model loaded with the diarizer still serves plain streams.
    let mut untagged = NativeNemotron::open(model, None, TRANSCRIPTION, false)
        .expect("plain stream on diar model");
    let (untagged_results, _) = stream(&mut untagged, &audio[..16_000 * 10]);
    assert!(
        untagged_results
            .iter()
            .all(|result| result.words.is_empty())
    );
}

#[test]
#[ignore = "requires the NeMo-Speech.cpp build and the ASR GGUF"]
fn a_stream_cannot_ask_for_labels_on_a_model_without_the_diarizer() {
    let paths = NemotronPaths::resolve();
    let model =
        NemotronModel::load(&paths, TRANSCRIPTION.rnnt_right_context, false).expect("ASR model");
    assert!(!model.has_diarization());
    assert!(NativeNemotron::open(model, None, TRANSCRIPTION, true).is_err());
}

/// The factory's swap: a plain recognizer is cached, the user turns labels
/// on, a diarizing recognizer is created while the plain one still exists,
/// and only then is the plain one released. Both must survive the overlap.
#[test]
#[ignore = "requires the NeMo-Speech.cpp build, both GGUFs and the AMI fixture"]
fn replacing_a_live_recognizer_with_a_diarizing_one_keeps_decoding() {
    let paths = NemotronPaths::resolve();
    let audio = ami_samples();
    let plain =
        NemotronModel::load(&paths, TRANSCRIPTION.rnnt_right_context, false).expect("ASR model");
    let mut stream_a = NativeNemotron::open(plain.clone(), None, TRANSCRIPTION, false).expect("a");
    let (first, _) = stream(&mut stream_a, &audio[..16_000 * 10]);
    assert!(first.iter().any(|result| result.is_final));
    let diarizing = NemotronModel::load(&paths, TRANSCRIPTION.rnnt_right_context, true)
        .expect("ASR + diarizer");
    drop(stream_a);
    drop(plain);
    let mut stream_b = NativeNemotron::open(diarizing, None, TRANSCRIPTION, true).expect("b");
    let (second, _) = stream(&mut stream_b, &audio[..16_000 * 20]);
    let tagged: usize = second
        .iter()
        .filter(|result| result.is_final)
        .map(|result| result.words.iter().filter(|word| word.speaker > 0).count())
        .sum();
    assert!(
        tagged > 0,
        "the diarizing recognizer must keep working after the swap"
    );
}
