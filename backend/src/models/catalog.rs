//! Every model the app knows how to verify and load, with provenance and license.

use super::{ModelKind, ModelProfile};

pub const RIVA_TRANSLATE_Q4: ModelProfile = ModelProfile {
    kind: ModelKind::Translation,
    id: "riva-translate-4b-instruct-v2-q4_k_m",
    directory: "riva-translate-4b-instruct-v2",
    filename: "riva-translate-4b-instruct-v2.q4_k_m.gguf",
    revision: "040d958b128018ff0bed2542a7b51005e9ea563c",
    source: "https://huggingface.co/nvidia/Riva-Translate-4B-Instruct-v2",
    license: "NVIDIA Open Model License",
    // SHA-256 of the GGUF requantized locally (llama-quantize --allow-requantize)
    // from the q8_0 conversion of the revision above. Regenerate straight from
    // the checkpoint before distributing.
    sha256: "decbb4d3b12f756cc384833fceeac10bf7699cd2286dc3e18a746af9c8df71af",
    max_bytes: 5 * 1024 * 1024 * 1024,
};

// OPUS-MT models run through CTranslate2 (int8, CPU). Each profile anchors the
// integrity check on the converted `model.bin`; the loader additionally expects
// `config.json`, `shared_vocabulary.json`, `source.spm` and `target.spm` in the
// same folder (see `translation::opus`). Converted locally with
// `ct2-transformers-converter --quantization int8` from the revisions below;
// regenerate straight from the checkpoints before distributing.

pub const OPUS_MT_EN_PT: ModelProfile = ModelProfile {
    kind: ModelKind::Translation,
    id: "opus-mt-tc-big-en-pt-ct2-int8",
    directory: "opus-mt-en-pt",
    filename: "model.bin",
    revision: "9f2863d807ecf91a374bdbecb8d01e402e90622e",
    source: "https://huggingface.co/Helsinki-NLP/opus-mt-tc-big-en-pt",
    license: "CC-BY-4.0",
    sha256: "5c2b607d9f0c00e935a00a497aa4c523225288697fbca57ddb7f9ee96fe80006",
    max_bytes: 512 * 1024 * 1024,
};

pub const OPUS_MT_PT_EN: ModelProfile = ModelProfile {
    kind: ModelKind::Translation,
    id: "opus-mt-tc-bible-big-roa-en-ct2-int8",
    directory: "opus-mt-pt-en",
    filename: "model.bin",
    revision: "4d4757865ab116b39daa3b4ca3cfe1e7daf43cbe",
    source: "https://huggingface.co/Helsinki-NLP/opus-mt-tc-bible-big-roa-en",
    license: "Apache-2.0",
    sha256: "b52ae8251ae34d6c8fd7bcc3d5202d6810ec6a8c005b89d762420f89d2f55fb2",
    max_bytes: 512 * 1024 * 1024,
};

// M2M100 418M (Meta, MIT): one many-to-many model for 100 languages, run
// through CTranslate2 int8 on the CPU like the OPUS models. The four files
// below are the int8 conversion published at
// https://huggingface.co/jncraton/m2m100_418M-ct2-int8 (also MIT), mirrored
// unchanged at https://huggingface.co/EduardoVinicius/anycaptions-m2m100 so
// the download does not depend on a repository the project does not control.
// The SHA-256 values were taken from the conversion on 2026-11-16 and checked
// again against the mirror. The download fetches all four into one folder.

/// Revision of the mirror every M2M100 file is pinned to.
pub const M2M100_REVISION: &str = "a7fece59547a8d6ceeb053db979d348016cc5671";

pub const M2M100_418M: ModelProfile = ModelProfile {
    kind: ModelKind::Translation,
    id: "m2m100-418m-ct2-int8",
    directory: "m2m100-418m",
    filename: "model.bin",
    revision: M2M100_REVISION,
    source: "https://huggingface.co/facebook/m2m100_418M",
    license: "MIT",
    sha256: "d6703dd9f920ff896e45c3d97b490761bed5944937b90bbe6a7245f5652542d4",
    max_bytes: 1024 * 1024 * 1024,
};

pub const M2M100_CONFIG: ModelProfile = ModelProfile {
    id: "m2m100-418m-config",
    filename: "config.json",
    sha256: "4244772990e30069563e3ddfb4ad6dc95bdfd2ac3de667ea8858c9b0a8433fa8",
    max_bytes: 64 * 1024,
    ..M2M100_418M
};

pub const M2M100_VOCABULARY: ModelProfile = ModelProfile {
    id: "m2m100-418m-vocabulary",
    filename: "shared_vocabulary.json",
    sha256: "7eb5d0ff184c6095c7c10f9911c0aea492250abd12854f9c3d787c64b1c6397e",
    max_bytes: 16 * 1024 * 1024,
    ..M2M100_418M
};

pub const M2M100_TOKENIZER: ModelProfile = ModelProfile {
    id: "m2m100-418m-tokenizer",
    filename: "sentencepiece.bpe.model",
    sha256: "d8f7c76ed2a5e0822be39f0a4f95a55eb19c78f4593ce609e2edbc2aea4d380a",
    max_bytes: 16 * 1024 * 1024,
    ..M2M100_418M
};

/// Streaming Sortformer (V3, eight speaker slots, 10 ms output frames) that
/// tags each recognized word with a generic speaker number. NVIDIA publishes
/// this GGUF already converted for NeMo-Speech.cpp (commit "add: q8_0 GGUF for
/// NeMo-Speech.cpp" in the model repository); the SHA-256 below is the Git
/// LFS object hash of that file, re-hashed locally on 2026-11-04. The model
/// knows no identities: it only separates voices into numbered slots.
pub const NEMOTRON_3_DIARIZATION_Q8: ModelProfile = ModelProfile {
    kind: ModelKind::Diarization,
    id: "nemotron-3-diarization-q8_0",
    directory: "nemotron-3-diarization",
    filename: "Nemotron-3-Diarization.q8_0.gguf",
    revision: "f667ed73aee57d40cc39428eb768b4fd87a0a29e",
    source: "https://huggingface.co/nvidia/Nemotron-3-Diarization",
    license: "OpenMDW-1.1",
    sha256: "08456d9e22cd9a323c0364d98375f3746d6e68507ebb705cd46438c534c7a3a1",
    max_bytes: 256 * 1024 * 1024,
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
