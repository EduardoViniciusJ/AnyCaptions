//! Integrity checks shared by every model in the catalog.

use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use sha2::{Digest, Sha256};

// One entry per verified file. It must hold every model the app can check at
// once: with a smaller cache each check evicted another model, so all of them
// were hashed again on every probe (measured 2026-11-16: ~3 s per round with
// the five catalog models installed).
const VERIFIED_CACHE_ENTRIES: usize = 32;

/// Suffix of the stamp written next to a model once its hash matched.
const STAMP_SUFFIX: &str = ".verified";
/// Read size while hashing; large reads keep the first check close to disk speed.
const HASH_BUFFER_BYTES: usize = 1024 * 1024;

type VerifiedFile = (PathBuf, u64, Option<SystemTime>, &'static str);
static VERIFIED_FILES: Mutex<Vec<VerifiedFile>> = Mutex::new(Vec::new());

/// Checks that `path` exists, is at most `max_bytes` and matches `sha256`.
///
/// The file is hashed once: the result is remembered by path, size and
/// modification time, in memory and in a small stamp next to the model, so an
/// unchanged file is not read again on later checks or later runs (hashing the
/// installed models took 1-13 s of every app start). A file that changes in
/// any way gets a new size or time and is hashed again. When the stamp cannot
/// be written (read-only folder) the file is simply hashed once per run.
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
    let stamp = stamp_text(path, sha256, len, entry.2);
    if let Some(stamp) = &stamp
        && fs::read_to_string(stamp_path(path)).is_ok_and(|saved| saved.trim() == stamp)
    {
        remember(entry);
        return Ok(());
    }
    let mut reader = file.take(max_bytes + 1);
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; HASH_BUFFER_BYTES];
    loop {
        let count = reader
            .read(&mut buffer)
            .map_err(|_| ModelProfileError::Corrupted)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    if !digest_matches(hasher, sha256) {
        let _ = fs::remove_file(stamp_path(path));
        return Err(ModelProfileError::Corrupted);
    }
    if let Some(stamp) = stamp {
        // Best effort: without the stamp the next run hashes the file again.
        let _ = fs::write(stamp_path(path), stamp);
    }
    remember(entry);
    Ok(())
}

/// Whether the finished hash equals `expected` (hexadecimal, any case).
/// Shared by everything that checks a download against a published SHA-256.
pub(crate) fn digest_matches(hasher: Sha256, expected: &str) -> bool {
    use std::fmt::Write;
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex.eq_ignore_ascii_case(expected)
}

fn remember(entry: VerifiedFile) {
    if let Ok(mut cache) = VERIFIED_FILES.lock() {
        cache.retain(|cached| cached.0 != entry.0);
        if cache.len() == VERIFIED_CACHE_ENTRIES {
            cache.remove(0);
        }
        cache.push(entry);
    }
}

fn stamp_path(model: &Path) -> PathBuf {
    let mut name = model.as_os_str().to_owned();
    name.push(STAMP_SUFFIX);
    PathBuf::from(name)
}

/// What the stamp must say for the file to count as already verified: the
/// expected hash, the exact size and modification time it was hashed at, and
/// where. The path keeps a stamp that travels with a copied or packaged model
/// folder from vouching for the copy, which is hashed on its own first use.
/// `None` when the file system reports no usable time; then nothing is stamped.
fn stamp_text(path: &Path, sha256: &str, len: u64, modified: Option<SystemTime>) -> Option<String> {
    let modified = modified?.duration_since(UNIX_EPOCH).ok()?;
    Some(format!(
        "{} {len} {} {}",
        sha256.to_ascii_lowercase(),
        modified.as_nanos(),
        path.display()
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelProfileError {
    Missing,
    Corrupted,
}

#[cfg(test)]
mod tests {
    use super::*;

    // SHA-256 of the bytes "model".
    const MODEL_SHA256: &str = "9372c470eeadd5ecd9c3c74c2b3cb633f8e2f2fad799250a0f70d652b6b825e4";

    #[test]
    fn digests_compare_as_hexadecimal_in_any_case() {
        let hashed = || {
            let mut hasher = Sha256::new();
            hasher.update(b"model");
            hasher
        };
        assert!(digest_matches(hashed(), MODEL_SHA256));
        assert!(digest_matches(hashed(), &MODEL_SHA256.to_ascii_uppercase()));
        assert!(!digest_matches(hashed(), &"0".repeat(64)));
        assert!(!digest_matches(hashed(), ""));
    }

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

    fn forget(path: &Path) {
        VERIFIED_FILES
            .lock()
            .unwrap()
            .retain(|cached| cached.0 != path);
    }

    #[test]
    fn a_verified_file_is_not_hashed_again_until_it_changes() {
        let path = std::env::temp_dir().join(format!(
            "anycaptions-stamped-model-{}.bin",
            std::process::id()
        ));
        std::fs::write(&path, b"model").unwrap();
        assert_eq!(verify_model_file(&path, MODEL_SHA256, 1024), Ok(()));
        let stamp = stamp_path(&path);
        assert!(stamp.is_file(), "the match is remembered next to the model");

        // A later run (empty memory cache) trusts the stamp: a wrong stamp
        // would be the only way this passes for a different expected hash.
        forget(&path);
        let saved = std::fs::read_to_string(&stamp).unwrap();
        assert!(saved.starts_with(MODEL_SHA256));

        // Changed content (new size) is hashed again and rejected, and the
        // stale stamp goes away.
        std::fs::write(&path, b"tampered-model").unwrap();
        assert_eq!(
            verify_model_file(&path, MODEL_SHA256, 1024),
            Err(ModelProfileError::Corrupted)
        );
        assert!(!stamp.exists());

        // A stamp for another hash never vouches for this file.
        std::fs::write(&path, b"model").unwrap();
        let other = "0".repeat(64);
        forget(&path);
        assert_eq!(verify_model_file(&path, MODEL_SHA256, 1024), Ok(()));
        forget(&path);
        assert_eq!(
            verify_model_file(&path, Box::leak(other.into_boxed_str()), 1024),
            Err(ModelProfileError::Corrupted)
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&stamp);
    }

    #[test]
    fn every_catalog_model_fits_in_the_cache_at_once() {
        let paths: Vec<PathBuf> = (0..8)
            .map(|index| {
                let path = std::env::temp_dir().join(format!(
                    "anycaptions-cache-model-{}-{index}.bin",
                    std::process::id()
                ));
                std::fs::write(&path, b"model").unwrap();
                assert_eq!(verify_model_file(&path, MODEL_SHA256, 1024), Ok(()));
                path
            })
            .collect();
        let cache = VERIFIED_FILES.lock().unwrap();
        for path in &paths {
            assert!(cache.iter().any(|cached| &cached.0 == path));
        }
        drop(cache);
        for path in paths {
            let _ = std::fs::remove_file(stamp_path(&path));
            let _ = std::fs::remove_file(path);
        }
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
