//! Catalog and on-disk layout of the machine-learning models the app distributes.
//! Files live under one root, grouped by capability: `models/<kind>/<model>/<file>`.

pub mod catalog;
mod verify;

use std::path::{Path, PathBuf};

pub use verify::ModelProfileError;

/// What a model does. Each kind owns a folder under the models root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelKind {
    Asr,
    Translation,
}

impl ModelKind {
    pub const fn dir_name(self) -> &'static str {
        match self {
            Self::Asr => "asr",
            Self::Translation => "translation",
        }
    }
}

/// Identity, provenance and integrity data for one distributed model file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProfile {
    pub kind: ModelKind,
    pub id: &'static str,
    /// Folder under the kind directory; quantization variants of a model share it.
    pub directory: &'static str,
    pub filename: &'static str,
    pub revision: &'static str,
    pub source: &'static str,
    pub license: &'static str,
    pub sha256: &'static str,
    pub max_bytes: u64,
}

impl ModelProfile {
    /// Full path of the model file under `models_root`.
    pub fn path_in(&self, models_root: &Path) -> PathBuf {
        models_root
            .join(self.kind.dir_name())
            .join(self.directory)
            .join(self.filename)
    }

    /// Checks that the file at `path` exists, stays within `max_bytes` and matches `sha256`.
    pub fn verify(&self, path: &Path) -> Result<(), ModelProfileError> {
        verify::verify_model_file(path, self.sha256, self.max_bytes)
    }
}

/// Models root: `ANYCAPTIONS_MODELS_DIR`, `backend/models` (debug builds only),
/// or `models/` next to the executable. Release builds skip the development path
/// so an installed app never reads models from the source tree.
pub fn models_root() -> PathBuf {
    let dev_models =
        cfg!(debug_assertions).then(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("models"));
    let exe_models = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("models")));
    [
        std::env::var_os("ANYCAPTIONS_MODELS_DIR").map(PathBuf::from),
        dev_models,
        exe_models.clone(),
    ]
    .into_iter()
    .flatten()
    .find(|dir| dir.is_dir())
    .unwrap_or_else(|| exe_models.unwrap_or_else(|| PathBuf::from("models")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_paths_are_grouped_by_kind_and_model_folder() {
        let path = catalog::NEMOTRON_STREAMING_Q8.path_in(Path::new("models"));
        assert_eq!(
            path,
            Path::new(
                "models/asr/nemotron-3.5-asr-streaming-0.6b/nemotron-3.5-asr-streaming-0.6b.q8_0.gguf"
            )
        );
    }

    #[test]
    fn catalog_profiles_carry_complete_integrity_data() {
        let profile = &catalog::NEMOTRON_STREAMING_Q8;
        assert_eq!(profile.sha256.len(), 64);
        assert!(profile.sha256.bytes().all(|b| b.is_ascii_hexdigit()));
        assert!(profile.max_bytes > 0);
    }
}
