use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::{Deserialize, Serialize};

use crate::dictation::DictationSettings;
use crate::domain::{AppSnapshot, Appearance, CaptionMode, TranslationEngine};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedGeometry {
    pub monitor: Option<String>,
    pub relative_x: f64,
    pub relative_y: f64,
    pub width: f64,
    pub height: f64,
}

impl SavedGeometry {
    fn valid(&self) -> bool {
        self.monitor.as_ref().is_none_or(|name| name.len() <= 256)
            && self.relative_x.is_finite()
            && (0.0..=1.0).contains(&self.relative_x)
            && self.relative_y.is_finite()
            && (0.0..=1.0).contains(&self.relative_y)
            && self.width.is_finite()
            && (1.0..=10000.0).contains(&self.width)
            && self.height.is_finite()
            && (1.0..=10000.0).contains(&self.height)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    pub mode: CaptionMode,
    #[serde(default)]
    pub translation_engine: TranslationEngine,
    #[serde(default)]
    pub ui_language: crate::domain::UiLanguage,
    pub output_id: Option<String>,
    pub appearance: Appearance,
    pub shortcut: String,
    pub geometry: Option<SavedGeometry>,
    #[serde(default)]
    pub dictation: DictationSettings,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            mode: CaptionMode::Auto,
            translation_engine: TranslationEngine::default(),
            ui_language: crate::domain::UiLanguage::default(),
            output_id: None,
            appearance: Appearance::default(),
            shortcut: "Ctrl+Alt+L".into(),
            geometry: None,
            dictation: DictationSettings::default(),
        }
    }
}

impl Preferences {
    fn valid(&self) -> bool {
        // A saved target must belong to the saved engine, or the restore would fail.
        let mode_supported = match self.mode {
            CaptionMode::Auto => true,
            CaptionMode::Translate(target) => self.translation_engine.supports(target),
        };
        mode_supported
            && self.appearance.validate().is_ok()
            && crate::domain::AppState::normalize_shortcut(&self.shortcut).is_ok()
            && self
                .output_id
                .as_ref()
                .is_none_or(|id| id.len() <= 512 && !id.is_empty())
            && self.geometry.as_ref().is_none_or(SavedGeometry::valid)
            && self.dictation.validate(&self.shortcut).is_ok()
    }
}

pub struct PreferenceStore {
    path: PathBuf,
    state: Mutex<Preferences>,
}

impl PreferenceStore {
    pub fn load(path: PathBuf) -> Self {
        let preferences = fs::read(&path)
            .ok()
            .filter(|bytes| bytes.len() <= 16 * 1024)
            .and_then(|bytes| serde_json::from_slice::<Preferences>(&bytes).ok())
            .filter(Preferences::valid)
            .unwrap_or_default();
        Self {
            path,
            state: Mutex::new(preferences),
        }
    }

    pub fn snapshot(&self) -> io::Result<Preferences> {
        self.state
            .lock()
            .map(|state| state.clone())
            .map_err(|_| io::Error::other("Preferences unavailable"))
    }

    pub fn sync_snapshot(&self, snapshot: &AppSnapshot) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("Preferences unavailable"))?;
        state.mode = snapshot.mode;
        state.translation_engine = snapshot.translation_engine;
        state.ui_language = snapshot.ui_language;
        state.appearance = snapshot.appearance;
        state.shortcut = snapshot.shortcut.clone();
        self.write(&state)
    }

    pub fn save_geometry(&self, geometry: SavedGeometry) -> io::Result<()> {
        if !geometry.valid() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid geometry",
            ));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("Preferences unavailable"))?;
        state.geometry = Some(geometry);
        self.write(&state)
    }

    pub fn save_dictation(&self, dictation: DictationSettings) -> io::Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("Preferences unavailable"))?;
        if dictation.validate(&state.shortcut).is_err() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid dictation settings",
            ));
        }
        state.dictation = dictation;
        self.write(&state)
    }

    pub fn save_output_id(&self, id: String) -> io::Result<()> {
        if id.is_empty() || id.len() > 512 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Invalid output ID",
            ));
        }
        let mut state = self
            .state
            .lock()
            .map_err(|_| io::Error::other("Preferences unavailable"))?;
        state.output_id = Some(id);
        self.write(&state)
    }

    fn write(&self, state: &Preferences) -> io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(state).map_err(io::Error::other)?;
        let temporary = self.path.with_extension("json.tmp");
        let mut file = fs::File::create(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(temporary, &self.path)
    }
}

pub fn config_path(root: &Path) -> PathBuf {
    root.join("preferences.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::CaptionColor;

    fn isolated() -> PathBuf {
        std::env::temp_dir().join(format!(
            "anycaptions-preferences-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn missing_invalid_and_oversized_data_use_defaults() {
        let dir = isolated();
        let path = config_path(&dir);
        assert_eq!(
            PreferenceStore::load(path.clone())
                .snapshot()
                .unwrap()
                .shortcut,
            "Ctrl+Alt+L"
        );
        fs::create_dir_all(&dir).unwrap();
        for data in ["{invalid", r#"{"mode":"pt-pt"}"#, r#"{"shortcut":"Win+L"}"#] {
            fs::write(&path, data).unwrap();
            assert_eq!(
                PreferenceStore::load(path.clone()).snapshot().unwrap().mode,
                CaptionMode::Auto
            );
        }
        fs::write(&path, vec![b'x'; 16 * 1024 + 1]).unwrap();
        assert_eq!(
            PreferenceStore::load(path.clone())
                .snapshot()
                .unwrap()
                .shortcut,
            "Ctrl+Alt+L"
        );
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn a_target_outside_the_saved_engine_falls_back_to_defaults() {
        let japanese = crate::domain::TargetLanguage::from_tag("ja").unwrap();
        let mut preferences = Preferences {
            mode: CaptionMode::Translate(japanese),
            translation_engine: TranslationEngine::Opus,
            ..Default::default()
        };
        assert!(!preferences.valid());
        preferences.translation_engine = TranslationEngine::Riva;
        assert!(preferences.valid());
    }

    #[test]
    fn dictation_preferences_restore_and_conflicts_fall_back_to_defaults() {
        let dir = isolated();
        let path = config_path(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut saved = Preferences {
            dictation: DictationSettings {
                enabled: true,
                shortcut: "F9".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
        let restored = PreferenceStore::load(path.clone()).snapshot().unwrap();
        assert!(restored.dictation.enabled);
        assert_eq!(restored.dictation.shortcut, "F9");
        // O atalho do ditado igual ao do overlay invalida o arquivo inteiro.
        saved.dictation.shortcut = saved.shortcut.clone();
        fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
        let restored = PreferenceStore::load(path.clone()).snapshot().unwrap();
        assert!(!restored.dictation.enabled);
        assert_eq!(restored.dictation.shortcut, "Ctrl+Space");
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn saves_only_preferences_and_restores_them() {
        let dir = isolated();
        let path = config_path(&dir);
        let store = PreferenceStore::load(path.clone());
        let mut snapshot = crate::domain::AppState::default().snapshot();
        snapshot.mode = CaptionMode::Translate(crate::domain::PT_BR);
        snapshot.translation_engine = TranslationEngine::Riva;
        snapshot.appearance = Appearance {
            font_size: 44,
            color: CaptionColor::YELLOW,
            partial_color: CaptionColor::DEFAULT_PARTIAL,
        };
        snapshot.shortcut = "Ctrl+Alt+K".into();
        store.sync_snapshot(&snapshot).unwrap();
        store
            .save_geometry(SavedGeometry {
                monitor: Some("DISPLAY1".into()),
                relative_x: 0.3,
                relative_y: 0.9,
                width: 780.0,
                height: 240.0,
            })
            .unwrap();
        let data = fs::read_to_string(&path).unwrap();
        assert!(!data.contains("caption") && !data.contains("audio") && !data.contains("session"));
        let restored = PreferenceStore::load(path.clone()).snapshot().unwrap();
        assert_eq!(restored.mode, CaptionMode::Translate(crate::domain::PT_BR));
        assert_eq!(restored.translation_engine, TranslationEngine::Riva);
        assert_eq!(restored.appearance.font_size, 44);
        assert_eq!(restored.appearance.color, CaptionColor::YELLOW);
        assert_eq!(
            restored.appearance.partial_color,
            CaptionColor::DEFAULT_PARTIAL
        );
        assert_eq!(restored.geometry.unwrap().relative_x, 0.3);
        fs::remove_file(path).unwrap();
        fs::remove_dir(dir).unwrap();
    }
}
