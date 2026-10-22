use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

use sha2::{Digest, Sha256};

const MAX_WHISPER_MODEL_BYTES: u64 = 512 * 1024 * 1024;
// A few model files may be checked per run; each entry avoids rehashing an unchanged file.
const VERIFIED_CACHE_ENTRIES: usize = 4;

type VerifiedFile = (PathBuf, u64, Option<SystemTime>, &'static str);
static VERIFIED_FILES: Mutex<Vec<VerifiedFile>> = Mutex::new(Vec::new());

/// Checks that `path` exists, is at most `max_bytes` and matches `sha256`. Results are cached
/// by path, size and modification time, so unchanged files are hashed once per run.
pub fn verify_model_file(
    path: &Path,
    sha256: &'static str,
    max_bytes: u64,
) -> Result<(), ModelProfileError> {
    let file = File::open(path).map_err(|_| ModelProfileError::Missing)?;
    let metadata = file.metadata().map_err(|_| ModelProfileError::Corrupted)?;
    let len = metadata.len();
    if len == 0 || len > max_bytes {
        return Err(ModelProfileError::Corrupted);
    }
    let entry = (path.to_path_buf(), len, metadata.modified().ok(), sha256);
    if VERIFIED_FILES
        .lock()
        .is_ok_and(|cache| cache.contains(&entry))
    {
        return Ok(());
    }
    let mut reader = file.take(max_bytes + 1);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 65_536];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| ModelProfileError::Corrupted)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    if !digest.eq_ignore_ascii_case(sha256) {
        return Err(ModelProfileError::Corrupted);
    }
    if let Ok(mut cache) = VERIFIED_FILES.lock() {
        cache.retain(|cached| cached.0 != entry.0);
        if cache.len() == VERIFIED_CACHE_ENTRIES {
            cache.remove(0);
        }
        cache.push(entry);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelProfileError {
    Missing,
    Corrupted,
    Incompatible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhisperModelProfile {
    pub id: &'static str,
    pub filename: &'static str,
    pub revision: &'static str,
    pub runtime: &'static str,
    pub source: &'static str,
    pub license: &'static str,
    pub sha256: &'static str,
    pub threads: i32,
    /// Use GPU if available (Vulkan / DirectML)
    pub use_gpu: bool,
    /// Enable flash‑attention (requires GPU backend)
    pub use_flash_attn: bool,
}

impl WhisperModelProfile {
    pub const fn active() -> Self {
        Self::base_en()
    }

    pub const fn small_en() -> Self {
        Self {
            id: "whisper-small.en",
            filename: "ggml-small.en.bin",
            revision: "5359861c739e955e79d9a303bcbc70fb988958b1",
            runtime: "whisper.cpp-cpu",
            source: "https://huggingface.co/ggerganov/whisper.cpp",
            license: "MIT",
            sha256: "c6138d6d58ecc8322097e0f987c32f1be8bb0a18532a3f88f734d1bbf9c41e5d",
            threads: 10,
            use_gpu: false,
            use_flash_attn: false,
        }
    }

    pub const fn base_en() -> Self {
        Self {
            id: "whisper-base.en",
            filename: "ggml-base.en.bin",
            revision: "5359861c739e955e79d9a303bcbc70fb988958b1",
            runtime: "whisper.cpp-cpu",
            source: "https://huggingface.co/ggerganov/whisper.cpp",
            license: "MIT",
            sha256: "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
            threads: 10,
            // GPU (Vulkan) is preferred; whisper.cpp falls back to CPU when no device is usable.
            use_gpu: true,
            use_flash_attn: false,
        }
    }

    pub const fn tiny_en() -> Self {
        Self {
            id: "whisper-tiny.en",
            filename: "ggml-tiny.en.bin",
            revision: "5359861c739e955e79d9a303bcbc70fb988958b1",
            runtime: "whisper.cpp-cpu",
            source: "https://huggingface.co/ggerganov/whisper.cpp",
            license: "MIT",
            sha256: "921e4cf8686fdd993dcd081a5da5b6c365bfde1162e72b08d75ac75289920b1f",
            threads: 10,
            use_gpu: false,
            use_flash_attn: false,
        }
    }

    pub fn verify(&self, path: &Path) -> Result<(), ModelProfileError> {
        verify_model_file(path, self.sha256, MAX_WHISPER_MODEL_BYTES)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // SHA-256 of the bytes "model".
    const MODEL_SHA256: &str = "9372c470eeadd5ecd9c3c74c2b3cb633f8e2f2fad799250a0f70d652b6b825e4";

    #[test]
    fn accepts_a_matching_model_file() {
        let path = std::env::temp_dir().join(format!(
            "anycaptions-valid-model-{}.bin",
            std::process::id()
        ));
        std::fs::write(&path, b"model").unwrap();
        assert_eq!(verify_model_file(&path, MODEL_SHA256, 1024), Ok(()));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn detects_missing_oversized_and_corrupted_model_files() {
        let missing = std::env::temp_dir().join("anycaptions-missing-model.bin");
        assert_eq!(
            verify_model_file(&missing, MODEL_SHA256, 1024),
            Err(ModelProfileError::Missing)
        );

        let path = std::env::temp_dir().join(format!(
            "anycaptions-corrupt-model-{}.bin",
            std::process::id()
        ));
        std::fs::write(&path, b"invalid-weights").unwrap();
        assert_eq!(
            verify_model_file(&path, MODEL_SHA256, 1024),
            Err(ModelProfileError::Corrupted)
        );
        assert_eq!(
            verify_model_file(&path, MODEL_SHA256, 4),
            Err(ModelProfileError::Corrupted)
        );
        let _ = std::fs::remove_file(path);
    }
}
