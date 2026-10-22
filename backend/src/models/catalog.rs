//! Every model the app knows how to verify and load, with provenance and license.

use super::{ModelKind, ModelProfile};

pub const RIVA_TRANSLATE_Q8: ModelProfile = ModelProfile {
    kind: ModelKind::Translation,
    id: "riva-translate-4b-instruct-v2-q8_0",
    directory: "riva-translate-4b-instruct-v2",
    filename: "riva-translate-4b-instruct-v2.q8_0.gguf",
    revision: "040d958b128018ff0bed2542a7b51005e9ea563c",
    source: "https://huggingface.co/nvidia/Riva-Translate-4B-Instruct-v2",
    license: "NVIDIA Open Model License",
    // SHA-256 of the GGUF converted locally from the revision above (convert_model.py, q8_0).
    sha256: "fb820b6e092e0dde67c53d8d5cface294c47fd67d0462df6a58f6e2bda9f2772",
    max_bytes: 5 * 1024 * 1024 * 1024,
};

pub const NEMOTRON_STREAMING_Q8: ModelProfile = ModelProfile {
    kind: ModelKind::Asr,
    id: "nemotron-3.5-asr-streaming-0.6b-q8_0",
    directory: "nemotron-3.5-asr-streaming-0.6b",
    filename: "nemotron-3.5-asr-streaming-0.6b.q8_0.gguf",
    revision: "ea30d66debe3740a08b573244286791d423d6b3e",
    source: "https://huggingface.co/nvidia/nemotron-3.5-asr-streaming-0.6b",
    license: "OpenMDW-1.1",
    sha256: "3fc991d3badad7277c11030a7519832cddaf2057aafed6d4b25147e953a070b1",
    max_bytes: 1024 * 1024 * 1024,
};
