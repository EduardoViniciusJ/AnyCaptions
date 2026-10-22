use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
    time::SystemTime,
};

use sha2::{Digest, Sha256};

const MAX_WHISPER_MODEL_BYTES: u64 = 512 * 1024 * 1024;

type VerifiedCache = (PathBuf, u64, Option<SystemTime>, &'static str);
static VERIFIED_WHISPER_CACHE: Mutex<Option<VerifiedCache>> = Mutex::new(None);

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
        Self::small_en()
    }

    // More accurate than base.en and slower; the encoder window follows the phrase length to
    // keep it fast on the CPU (see `audio_ctx_for` in whisper.rs).
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
            // The default build compiles no GPU backend, so inference runs on the CPU.
            use_gpu: false,
            use_flash_attn: false,
        }
    }

    pub fn verify(&self, path: &Path) -> Result<(), ModelProfileError> {
        let file = File::open(path).map_err(|_| ModelProfileError::Missing)?;
        let metadata = file.metadata().map_err(|_| ModelProfileError::Corrupted)?;
        let len = metadata.len();
        if len == 0 || len > MAX_WHISPER_MODEL_BYTES {
            return Err(ModelProfileError::Corrupted);
        }
        let modified = metadata.modified().ok();
        if let Ok(guard) = VERIFIED_WHISPER_CACHE.lock()
            && let Some((cached_path, cached_len, cached_mod, cached_sha)) = guard.as_ref()
            && cached_path == path
            && *cached_len == len
            && *cached_mod == modified
            && *cached_sha == self.sha256
        {
            return Ok(());
        }
        let mut reader = file.take(MAX_WHISPER_MODEL_BYTES + 1);
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
        if !digest.eq_ignore_ascii_case(self.sha256) {
            return Err(ModelProfileError::Corrupted);
        }
        if let Ok(mut guard) = VERIFIED_WHISPER_CACHE.lock() {
            *guard = Some((path.to_path_buf(), len, modified, self.sha256));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_missing_and_corrupted_whisper_model_files() {
        let profile = WhisperModelProfile::base_en();
        let missing = std::env::temp_dir().join("anycaptions-missing-whisper.bin");
        assert_eq!(profile.verify(&missing), Err(ModelProfileError::Missing));

        let corrupt = std::env::temp_dir().join(format!(
            "anycaptions-corrupt-whisper-{}.bin",
            std::process::id()
        ));
        std::fs::write(&corrupt, b"invalid-ggml-weights").unwrap();
        assert_eq!(profile.verify(&corrupt), Err(ModelProfileError::Corrupted));
        let _ = std::fs::remove_file(corrupt);
    }
}
