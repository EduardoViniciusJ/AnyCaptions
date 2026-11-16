//! Latency benchmark for every engine on the real hardware.
//! Run explicitly (release build, models and DLLs from an installed app):
//!   ANYCAPTIONS_NEMO_DIR=<install> ANYCAPTIONS_MODELS_DIR=<install>/models \
//!   cargo test --release --test bench_latency -- --ignored --nocapture --test-threads=1
//! Each phase prints a wall-clock stamp so an external VRAM/RAM sampler can be
//! lined up with what was loaded at the time.

use std::{
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use anycaptions_lib::{
    transcription::{
        nemotron::{NativeNemotron, NemotronPaths},
        streaming::StreamingRecognizer,
        tuning::{TRANSCRIPTION, TRANSLATION},
    },
    translation::{
        Translator,
        opus::{NativeOpusTranslator, OpusPaths},
        riva::{NativeRivaTranslator, RivaDevice, RivaPaths},
    },
};

const BLOCK_SAMPLES: usize = 2_048; // 128 ms, same as the capture coordinator

const ENGLISH: &[&str] = &[
    "Ask not what your country can do for you.",
    "The update will be released next week.",
    "I can't believe we won that match!",
    "Press the button to start the game.",
    "Thanks for watching, see you tomorrow.",
    "We need to push B site right now, they are rotating.",
    "This is probably the best episode of the whole season.",
    "Can you hear me? I think my microphone is broken.",
];

fn stamp(label: &str) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    eprintln!("[stamp] {secs:.1} {label}");
}

fn wav(name: &str) -> Vec<f32> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benchmarks/local")
        .join(name);
    let mut reader = hound::WavReader::open(path).expect("fixture wav");
    reader
        .samples::<i16>()
        .map(|s| f32::from(s.unwrap()) / 32_768.0)
        .collect()
}

fn stats(label: &str, mut ms: Vec<f64>) {
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let avg = ms.iter().sum::<f64>() / ms.len() as f64;
    let p95 = ms[((ms.len() as f64 * 0.95).ceil() as usize).min(ms.len()) - 1];
    eprintln!(
        "[bench] {label}: n={} avg={avg:.1}ms p95={p95:.1}ms max={:.1}ms",
        ms.len(),
        ms.last().unwrap()
    );
}

fn bench_asr(label: &str, rec: &mut NativeNemotron, samples: &[f32]) {
    let mut per_block = Vec::new();
    let mut finals = Vec::new();
    let total = Instant::now();
    for block in samples.chunks(BLOCK_SAMPLES) {
        let t = Instant::now();
        let out = rec.push(block).unwrap();
        per_block.push(t.elapsed().as_secs_f64() * 1000.0);
        finals.extend(out.into_iter().filter(|r| r.is_final).map(|r| r.text));
    }
    finals.extend(
        rec.finish()
            .unwrap()
            .into_iter()
            .filter(|r| r.is_final)
            .map(|r| r.text),
    );
    let total_ms = total.elapsed().as_secs_f64() * 1000.0;
    let audio_ms = samples.len() as f64 / 16.0;
    stats(&format!("asr {label} per 128ms block"), per_block);
    eprintln!(
        "[bench] asr {label}: audio={audio_ms:.0}ms compute={total_ms:.0}ms RTF={:.3}",
        total_ms / audio_ms
    );
    eprintln!("[bench] asr {label} text: {}", finals.join(" | "));
}

fn bench_translator(label: &str, tr: &mut dyn Translator) {
    // First call after load is excluded (pipeline compilation).
    let _ = tr.translate("Ok.", "en-US", "pt-BR");
    let mut ms = Vec::new();
    for s in ENGLISH {
        let t = Instant::now();
        let out = tr.translate(s, "en-US", "pt-BR").unwrap();
        ms.push(t.elapsed().as_secs_f64() * 1000.0);
        eprintln!("[bench] {label}: {s} -> {out}");
    }
    stats(&format!("{label} en->pt per sentence"), ms);
}

#[test]
#[ignore = "benchmark: needs DLLs, models and benchmarks/local/*.wav"]
fn bench_all_engines() {
    let pause = || std::thread::sleep(std::time::Duration::from_secs(4));
    let en = wav("jfk.wav");
    let pt = wav("pt.wav");

    stamp("asr load start");
    let t = Instant::now();
    let model = anycaptions_lib::transcription::nemotron::NemotronModel::load(
        &NemotronPaths::resolve(),
        None,
        false,
    )
    .expect("asr");
    eprintln!("[bench] asr load_ms={}", t.elapsed().as_millis());
    stamp("asr loaded");
    pause();
    for (label, tuning) in [
        ("transcription", TRANSCRIPTION),
        ("translation", TRANSLATION),
    ] {
        let mut rec = NativeNemotron::open(std::sync::Arc::clone(&model), None, tuning, false)
            .expect("stream");
        bench_asr(&format!("{label} en"), &mut rec, &en);
        let mut rec = NativeNemotron::open(std::sync::Arc::clone(&model), None, tuning, false)
            .expect("stream");
        bench_asr(&format!("{label} pt"), &mut rec, &pt);
    }

    stamp("opus load start");
    let t = Instant::now();
    let mut opus = NativeOpusTranslator::load(&OpusPaths::resolve()).expect("opus");
    eprintln!("[bench] opus load_ms={}", t.elapsed().as_millis());
    stamp("opus loaded");
    bench_translator("opus", &mut opus);
    drop(opus);

    for (label, device) in [
        ("riva-gpu", RivaDevice::Auto),
        ("riva-cpu", RivaDevice::Cpu),
    ] {
        stamp(&format!("{label} load start"));
        let t = Instant::now();
        let mut riva = NativeRivaTranslator::load_on(&RivaPaths::resolve(), device).expect("riva");
        eprintln!("[bench] {label} load_ms={}", t.elapsed().as_millis());
        stamp(&format!("{label} loaded"));
        pause();
        bench_translator(label, &mut riva);
        drop(riva);
        stamp(&format!("{label} dropped"));
        pause();
    }
    drop(model);
    stamp("all dropped");
}
