use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WebviewWindow};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::audio::devices::{AudioDeviceError, OutputSelection, OutputService};
use crate::domain::{AppSnapshot, AppState, Appearance, CaptionMode, DomainError, WindowMode};
use crate::preferences::{PreferenceStore, Preferences};

pub const STATE_EVENT: &str = "app-state-changed";
pub const CAPTION_EVENT: &str = "caption-updated";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionUpdate {
    pub session_id: u32,
    pub segment_id: u32,
    pub revision: u32,
    pub mode: CaptionMode,
    pub text: String,
    pub is_final: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "code", rename_all = "kebab-case")]
pub enum CommandError {
    Domain(DomainError),
    StateUnavailable,
    EventUnavailable,
    UnauthorizedWindow,
    WindowUnavailable,
    ShortcutUnavailable,
    PreferencesUnavailable,
    AudioDevice(AudioDeviceError),
}

impl From<DomainError> for CommandError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
    }
}

impl From<AudioDeviceError> for CommandError {
    fn from(value: AudioDeviceError) -> Self {
        Self::AudioDevice(value)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeRequest {
    pub mode: CaptionMode,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowRequest {
    pub mode: WindowMode,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShortcutRequest {
    pub shortcut: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputRequest {
    pub id: String,
}

pub trait StateEvents {
    fn state_changed(&self, state: &AppSnapshot) -> Result<(), CommandError>;
}

trait ShortcutRegistry {
    fn register(&self, shortcut: &str) -> Result<(), CommandError>;
    fn unregister(&self, shortcut: &str) -> Result<(), CommandError>;
}

struct NativeShortcuts<'a, R: Runtime>(&'a AppHandle<R>);

impl<R: Runtime> ShortcutRegistry for NativeShortcuts<'_, R> {
    fn register(&self, shortcut: &str) -> Result<(), CommandError> {
        self.0
            .global_shortcut()
            .on_shortcut(shortcut, |app, _, event| {
                if event.state == ShortcutState::Pressed {
                    crate::recovery::toggle(app);
                }
            })
            .map_err(|_| CommandError::ShortcutUnavailable)
    }

    fn unregister(&self, shortcut: &str) -> Result<(), CommandError> {
        self.0
            .global_shortcut()
            .unregister(shortcut)
            .map_err(|_| CommandError::ShortcutUnavailable)
    }
}

fn replace_shortcut(
    registry: &impl ShortcutRegistry,
    previous: &AppSnapshot,
    requested: &str,
) -> Result<(), CommandError> {
    if requested == previous.shortcut && previous.shortcut_available {
        return Ok(());
    }
    registry.register(requested)?;
    if previous.shortcut_available
        && previous.shortcut != requested
        && registry.unregister(&previous.shortcut).is_err()
    {
        let _ = registry.unregister(requested);
        return Err(CommandError::ShortcutUnavailable);
    }
    Ok(())
}

impl<R: Runtime> StateEvents for WebviewWindow<R> {
    fn state_changed(&self, state: &AppSnapshot) -> Result<(), CommandError> {
        self.emit_to(self.label(), STATE_EVENT, state)
            .map_err(|_| CommandError::EventUnavailable)
    }
}

#[derive(Default)]
pub struct Controller(Mutex<AppState>, Mutex<()>);

impl Controller {
    pub fn restore_preferences(&self, preferences: &Preferences) -> Result<(), CommandError> {
        let mut state = self.0.lock().map_err(|_| CommandError::StateUnavailable)?;
        state.set_mode(preferences.mode)?;
        state.set_appearance(preferences.appearance)?;
        state.set_shortcut(preferences.shortcut.clone(), false)?;
        Ok(())
    }

    pub fn snapshot(&self) -> Result<AppSnapshot, CommandError> {
        Ok(self
            .0
            .lock()
            .map_err(|_| CommandError::StateUnavailable)?
            .snapshot())
    }

    fn update(
        &self,
        events: &impl StateEvents,
        action: impl FnOnce(&mut AppState) -> Result<(), DomainError>,
    ) -> Result<AppSnapshot, CommandError> {
        let (snapshot, changed) = {
            let mut state = self.0.lock().map_err(|_| CommandError::StateUnavailable)?;
            let previous_revision = state.snapshot().revision;
            action(&mut state)?;
            let snapshot = state.snapshot();
            let changed = snapshot.revision != previous_revision;
            (snapshot, changed)
        };
        // The authoritative state is committed even if delivery fails; callers can re-query it.
        if changed {
            events.state_changed(&snapshot)?;
        }
        Ok(snapshot)
    }

    pub fn enable_tray(&self, window: &impl StateEvents) -> Result<AppSnapshot, CommandError> {
        self.update(window, |state| state.set_recovery(true, false))
    }

    pub fn set_shortcut(
        &self,
        shortcut: String,
        available: bool,
        window: &impl StateEvents,
    ) -> Result<AppSnapshot, CommandError> {
        self.update(window, |state| state.set_shortcut(shortcut, available))
    }

    pub fn change_shortcut<R: Runtime>(
        &self,
        app: &AppHandle<R>,
        window: &WebviewWindow<R>,
        requested: &str,
    ) -> Result<AppSnapshot, CommandError> {
        let normalized = AppState::normalize_shortcut(requested)?;
        let _change = self.1.lock().map_err(|_| CommandError::StateUnavailable)?;
        let previous = self.snapshot()?;
        if normalized == previous.shortcut && previous.shortcut_available {
            return Ok(previous);
        }
        replace_shortcut(&NativeShortcuts(app), &previous, &normalized)?;
        self.set_shortcut(normalized, true, window)
    }

    pub fn change_window_mode<R: Runtime>(
        &self,
        window: &WebviewWindow<R>,
        mode: WindowMode,
    ) -> Result<AppSnapshot, CommandError> {
        let (snapshot, changed) = {
            let mut state = self.0.lock().map_err(|_| CommandError::StateUnavailable)?;
            let mut candidate = state.clone();
            candidate.set_window_mode(mode)?;
            let changed = candidate.snapshot().revision != state.snapshot().revision;
            if changed {
                window
                    .set_ignore_cursor_events(mode == WindowMode::Locked)
                    .map_err(|_| CommandError::WindowUnavailable)?;
                *state = candidate;
            }
            (state.snapshot(), changed)
        };
        if changed {
            window.state_changed(&snapshot)?;
        }
        if mode == WindowMode::Edit {
            crate::window::ensure_visible(window).map_err(|_| CommandError::WindowUnavailable)?;
            window.show().map_err(|_| CommandError::WindowUnavailable)?;
        }
        Ok(snapshot)
    }
}

fn persist<R: Runtime>(app: &AppHandle<R>, state: &AppSnapshot) -> Result<(), CommandError> {
    if let Some(store) = app.try_state::<PreferenceStore>() {
        store
            .sync_snapshot(state)
            .map_err(|_| CommandError::PreferencesUnavailable)?;
    }
    Ok(())
}

fn authorize(label: &str) -> Result<(), CommandError> {
    if label != "overlay" {
        return Err(CommandError::UnauthorizedWindow);
    }
    Ok(())
}

#[tauri::command]
pub fn get_state<R: Runtime>(
    window: WebviewWindow<R>,
    controller: State<'_, Controller>,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    controller.snapshot()
}

#[tauri::command]
pub fn set_caption_mode<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    controller: State<'_, Controller>,
    request: ModeRequest,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    let state = controller.update(&window, |state| state.set_mode(request.mode))?;
    persist(&app, &state)?;
    Ok(state)
}

#[tauri::command]
pub fn start_capture<R: Runtime>(
    window: WebviewWindow<R>,
    controller: State<'_, Controller>,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    controller.update(&window, AppState::start)
}

#[tauri::command]
pub fn stop_capture<R: Runtime>(
    window: WebviewWindow<R>,
    controller: State<'_, Controller>,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    controller.update(&window, AppState::stop)
}

#[tauri::command]
pub fn set_appearance<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    controller: State<'_, Controller>,
    request: Appearance,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    let state = controller.update(&window, |state| state.set_appearance(request))?;
    persist(&app, &state)?;
    Ok(state)
}

#[tauri::command]
pub fn set_window_mode<R: Runtime>(
    window: WebviewWindow<R>,
    controller: State<'_, Controller>,
    request: WindowRequest,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    controller.change_window_mode(&window, request.mode)
}

#[tauri::command]
pub fn set_shortcut<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    controller: State<'_, Controller>,
    request: ShortcutRequest,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    let state = controller.change_shortcut(&app, &window, &request.shortcut)?;
    persist(&app, &state)?;
    Ok(state)
}

#[tauri::command]
pub async fn get_audio_outputs<R: Runtime>(
    window: WebviewWindow<R>,
    service: State<'_, Arc<OutputService>>,
) -> Result<OutputSelection, CommandError> {
    authorize(window.label())?;
    let service = Arc::clone(&service);
    tauri::async_runtime::spawn_blocking(move || service.selection())
        .await
        .map_err(|_| CommandError::StateUnavailable)?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn select_audio_output<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    service: State<'_, Arc<OutputService>>,
    request: OutputRequest,
) -> Result<OutputSelection, CommandError> {
    authorize(window.label())?;
    let service = Arc::clone(&service);
    tauri::async_runtime::spawn_blocking(move || {
        let selection = service.select(&request.id)?;
        app.state::<PreferenceStore>()
            .save_output_id(request.id)
            .map_err(|_| CommandError::PreferencesUnavailable)?;
        Ok(selection)
    })
    .await
    .map_err(|_| CommandError::StateUnavailable)?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::CaptionColor;
    use std::cell::RefCell;
    use std::collections::HashSet;

    #[derive(Default)]
    struct Events(RefCell<Vec<AppSnapshot>>);
    impl StateEvents for Events {
        fn state_changed(&self, state: &AppSnapshot) -> Result<(), CommandError> {
            self.0.borrow_mut().push(state.clone());
            Ok(())
        }
    }

    #[test]
    fn successful_changes_emit_the_committed_state() {
        let controller = Controller::default();
        let events = Events::default();
        let response = controller
            .update(&events, |state| state.set_mode(CaptionMode::EnPt))
            .unwrap();
        let recorded = events.0.borrow();
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].mode, CaptionMode::EnPt);
        assert_eq!(recorded[0].session_id, response.session_id);
        assert_eq!(
            recorded[0].revision,
            controller.snapshot().unwrap().revision
        );
    }

    #[test]
    fn rejected_or_unchanged_requests_emit_nothing() {
        let controller = Controller::default();
        let events = Events::default();
        controller.update(&events, AppState::stop).unwrap();
        assert!(controller.update(&events, AppState::start).is_err());
        assert!(
            controller
                .update(&events, |state| state.set_appearance(Appearance {
                    font_size: 100,
                    color: CaptionColor::White
                }))
                .is_err()
        );
        assert!(events.0.borrow().is_empty());
        assert_eq!(controller.snapshot().unwrap().revision, 0);
    }

    #[test]
    fn an_event_failure_can_be_recovered_by_reading_state() {
        struct BrokenEvents;
        impl StateEvents for BrokenEvents {
            fn state_changed(&self, _: &AppSnapshot) -> Result<(), CommandError> {
                Err(CommandError::EventUnavailable)
            }
        }
        let controller = Controller::default();
        let result = controller.update(&BrokenEvents, |state| state.set_mode(CaptionMode::EnPt));
        assert_eq!(result.unwrap_err(), CommandError::EventUnavailable);
        assert_eq!(controller.snapshot().unwrap().mode, CaptionMode::EnPt);
    }

    #[test]
    fn only_the_overlay_can_call_commands() {
        assert!(authorize("overlay").is_ok());
        for label in ["main", "remote", "overlay-child", ""] {
            assert_eq!(authorize(label), Err(CommandError::UnauthorizedWindow));
        }
    }

    #[test]
    fn ipc_requests_reject_unsupported_modes_and_extra_fields() {
        assert!(serde_json::from_str::<ModeRequest>(r#"{"mode":"auto"}"#).is_err());
        assert!(serde_json::from_str::<ModeRequest>(r#"{"mode":"en-en","audio":[1,2]}"#).is_err());
        assert!(serde_json::from_str::<WindowRequest>(r#"{"mode":"hidden"}"#).is_err());
    }

    #[test]
    fn event_contracts_use_frontend_field_names() {
        let update = CaptionUpdate {
            session_id: 1,
            segment_id: 2,
            revision: 3,
            mode: CaptionMode::EnPt,
            text: "<b>texto</b>".into(),
            is_final: true,
        };
        let json = serde_json::to_value(update).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"sessionId":1,"segmentId":2,"revision":3,"mode":"en-pt","text":"<b>texto</b>","isFinal":true})
        );
        let snapshot = serde_json::to_value(AppState::default().snapshot()).unwrap();
        assert_eq!(snapshot["windowMode"], "edit");
        assert_eq!(snapshot["appearance"]["fontSize"], 32);
    }

    #[test]
    fn shortcut_conflict_keeps_the_previous_recovery_chord() {
        struct FakeRegistry {
            registered: RefCell<HashSet<String>>,
            blocked: Option<&'static str>,
            stuck: Option<&'static str>,
        }
        impl ShortcutRegistry for FakeRegistry {
            fn register(&self, key: &str) -> Result<(), CommandError> {
                if self.blocked == Some(key) {
                    return Err(CommandError::ShortcutUnavailable);
                }
                self.registered.borrow_mut().insert(key.into());
                Ok(())
            }
            fn unregister(&self, key: &str) -> Result<(), CommandError> {
                if self.stuck == Some(key) {
                    return Err(CommandError::ShortcutUnavailable);
                }
                self.registered.borrow_mut().remove(key);
                Ok(())
            }
        }
        let mut previous = AppState::default().snapshot();
        previous.shortcut_available = true;
        let registry = FakeRegistry {
            registered: RefCell::new(HashSet::from([previous.shortcut.clone()])),
            blocked: Some("Ctrl+Alt+K"),
            stuck: None,
        };
        assert_eq!(
            replace_shortcut(&registry, &previous, "Ctrl+Alt+K"),
            Err(CommandError::ShortcutUnavailable)
        );
        assert_eq!(registry.registered.borrow().len(), 1);
        assert!(registry.registered.borrow().contains("Ctrl+Alt+L"));

        let registry = FakeRegistry {
            registered: RefCell::new(HashSet::from([previous.shortcut.clone()])),
            blocked: None,
            stuck: None,
        };
        replace_shortcut(&registry, &previous, "Ctrl+Alt+K").unwrap();
        assert_eq!(registry.registered.borrow().len(), 1);
        assert!(registry.registered.borrow().contains("Ctrl+Alt+K"));

        let registry = FakeRegistry {
            registered: RefCell::new(HashSet::from([previous.shortcut.clone()])),
            blocked: None,
            stuck: Some("Ctrl+Alt+L"),
        };
        assert_eq!(
            replace_shortcut(&registry, &previous, "Ctrl+Alt+K"),
            Err(CommandError::ShortcutUnavailable)
        );
        assert_eq!(registry.registered.borrow().len(), 1);
        assert!(registry.registered.borrow().contains("Ctrl+Alt+L"));
    }
}
