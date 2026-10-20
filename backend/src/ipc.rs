use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{Emitter, Runtime, State, WebviewWindow};

use crate::domain::{AppSnapshot, AppState, Appearance, CaptionMode, DomainError, WindowMode};

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
}

impl From<DomainError> for CommandError {
    fn from(value: DomainError) -> Self {
        Self::Domain(value)
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

pub trait StateEvents {
    fn state_changed(&self, state: &AppSnapshot) -> Result<(), CommandError>;
}

impl<R: Runtime> StateEvents for WebviewWindow<R> {
    fn state_changed(&self, state: &AppSnapshot) -> Result<(), CommandError> {
        self.emit_to(self.label(), STATE_EVENT, state)
            .map_err(|_| CommandError::EventUnavailable)
    }
}

#[derive(Default)]
pub struct Controller(Mutex<AppState>);

impl Controller {
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
    controller: State<'_, Controller>,
    request: ModeRequest,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    controller.update(&window, |state| state.set_mode(request.mode))
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
    controller: State<'_, Controller>,
    request: Appearance,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    controller.update(&window, |state| state.set_appearance(request))
}

#[tauri::command]
pub fn set_window_mode<R: Runtime>(
    window: WebviewWindow<R>,
    controller: State<'_, Controller>,
    request: WindowRequest,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    controller.update(&window, |state| state.set_window_mode(request.mode))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::CaptionColor;
    use std::cell::RefCell;

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
}
