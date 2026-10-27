use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WebviewWindow};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::audio::devices::{AudioDeviceError, DeviceSelection, InputService, OutputService};
use crate::dictation::{DictationError, DictationSettings, runtime as dictation_runtime};
use crate::domain::{
    AppSnapshot, AppState, Appearance, CaptionMode, CaptureState, DomainError, Resources,
    TranslationEngine, WindowMode,
};
use crate::models::{
    catalog,
    download::{self, DownloadManager, DownloadSink, DownloadUpdate},
};
use crate::preferences::{PreferenceStore, Preferences};
use crate::transcription::session::{CaptionSessionManager, SessionSink};

pub const STATE_EVENT: &str = "app-state-changed";
pub const CAPTION_EVENT: &str = "caption-updated";
pub const MODEL_DOWNLOAD_EVENT: &str = "model-download-changed";

impl<R: Runtime> DownloadSink for WebviewWindow<R> {
    fn download_changed(&self, update: &DownloadUpdate) {
        let _ = self.emit_to(self.label(), MODEL_DOWNLOAD_EVENT, update);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionUpdate {
    pub session_id: u32,
    pub segment_id: u32,
    pub revision: u32,
    pub mode: CaptionMode,
    pub text: String,
    pub is_final: bool,
    /// Language detected by the model (BCP-47); only final captions carry it.
    pub language: Option<String>,
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
    Dictation(DictationError),
    BrowserUnavailable,
    DownloadBusy,
    DownloadUnavailable,
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

impl From<DictationError> for CommandError {
    fn from(value: DictationError) -> Self {
        Self::Dictation(value)
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModeRequest {
    pub mode: CaptionMode,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiLanguageRequest {
    pub language: crate::domain::UiLanguage,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TranslationEngineRequest {
    pub engine: crate::domain::TranslationEngine,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceLanguageRequest {
    /// `None` volta à detecção automática; a tag é validada pelo domínio.
    pub language: Option<crate::domain::SourceLanguage>,
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

#[derive(Clone, Default)]
pub struct Controller(Arc<(Mutex<AppState>, Mutex<()>)>);

impl Controller {
    pub fn restore_preferences(&self, preferences: &Preferences) -> Result<(), CommandError> {
        let mut state = self
            .0
            .0
            .lock()
            .map_err(|_| CommandError::StateUnavailable)?;
        // Engine first: a saved translate mode is only valid under its engine.
        state.set_translation_engine(preferences.translation_engine)?;
        state.set_mode(preferences.mode)?;
        state.set_source_language(preferences.source_language)?;
        state.set_ui_language(preferences.ui_language)?;
        state.set_appearance(preferences.appearance)?;
        state.set_shortcut(preferences.shortcut.clone(), false)?;
        Ok(())
    }

    pub fn snapshot(&self) -> Result<AppSnapshot, CommandError> {
        Ok(self
            .0
            .0
            .lock()
            .map_err(|_| CommandError::StateUnavailable)?
            .snapshot())
    }

    fn update(
        &self,
        events: &(impl StateEvents + ?Sized),
        action: impl FnOnce(&mut AppState) -> Result<(), DomainError>,
    ) -> Result<AppSnapshot, CommandError> {
        let (snapshot, changed) = {
            let mut state = self
                .0
                .0
                .lock()
                .map_err(|_| CommandError::StateUnavailable)?;
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

    pub fn set_resources(
        &self,
        resources: Resources,
        events: &(impl StateEvents + ?Sized),
    ) -> Result<AppSnapshot, CommandError> {
        self.update(events, |state| state.set_resources(resources))
    }

    pub fn start_session(
        &self,
        events: &(impl StateEvents + ?Sized),
    ) -> Result<AppSnapshot, CommandError> {
        self.update(events, AppState::start)
    }

    pub fn stop_session(
        &self,
        events: &(impl StateEvents + ?Sized),
    ) -> Result<AppSnapshot, CommandError> {
        self.update(events, AppState::stop)
    }

    pub fn transition_session(
        &self,
        session_id: u32,
        next: CaptureState,
        events: &(impl StateEvents + ?Sized),
    ) -> Result<AppSnapshot, CommandError> {
        self.update(events, |state| state.transition(session_id, next))
    }

    pub fn fail_session(
        &self,
        session_id: u32,
        error: DomainError,
        events: &(impl StateEvents + ?Sized),
    ) -> Result<AppSnapshot, CommandError> {
        self.update(events, |state| state.fail(session_id, error))
    }

    pub fn is_session_active(&self, session_id: u32) -> bool {
        self.snapshot().is_ok_and(|snapshot| {
            snapshot.session_id == session_id
                && matches!(
                    snapshot.capture,
                    CaptureState::Preparing | CaptureState::Listening | CaptureState::Processing
                )
        })
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
        let _change = self
            .0
            .1
            .lock()
            .map_err(|_| CommandError::StateUnavailable)?;
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
            let mut state = self
                .0
                .0
                .lock()
                .map_err(|_| CommandError::StateUnavailable)?;
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

pub fn start_live_capture<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
    controller: &Controller,
) -> Result<AppSnapshot, CommandError> {
    if let (Some(sessions), Some(outputs)) = (
        app.try_state::<CaptionSessionManager>(),
        app.try_state::<Arc<OutputService>>(),
    ) {
        let _ = sessions.refresh_resources(controller, &outputs, window);
        let selection = outputs.selection()?;
        let snapshot = controller.start_session(window)?;
        let Some(device_id) = selection
            .selected_id
            .filter(|_| selection.selected_available)
        else {
            let _ = controller.fail_session(
                snapshot.session_id,
                DomainError::CaptureUnavailable,
                window,
            );
            return Err(CommandError::Domain(DomainError::CaptureUnavailable));
        };
        let sink: Arc<dyn SessionSink> = Arc::new(window.clone());
        if let Err(err) = sessions.start_session(
            crate::transcription::session::SessionPlan {
                session_id: snapshot.session_id,
                mode: snapshot.mode,
                engine: snapshot.translation_engine,
                source_language: snapshot.source_language,
            },
            device_id,
            controller.clone(),
            sink,
        ) {
            let _ = controller.fail_session(
                snapshot.session_id,
                DomainError::CaptureUnavailable,
                window,
            );
            return Err(err);
        }
        return Ok(snapshot);
    }
    controller.start_session(window)
}

pub fn stop_live_capture<R: Runtime>(
    app: &AppHandle<R>,
    window: &WebviewWindow<R>,
    controller: &Controller,
) -> Result<AppSnapshot, CommandError> {
    if let Some(sessions) = app.try_state::<CaptionSessionManager>() {
        sessions.stop_session();
    }
    controller.stop_session(window)
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
    if let Some(sessions) = app.try_state::<CaptionSessionManager>() {
        sessions.stop_session();
    }
    let state = controller.update(&window, |state| state.set_mode(request.mode))?;
    persist(&app, &state)?;
    Ok(state)
}

#[tauri::command]
pub fn set_source_language<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    controller: State<'_, Controller>,
    request: SourceLanguageRequest,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    if let Some(sessions) = app.try_state::<CaptionSessionManager>() {
        sessions.stop_session();
    }
    let state = controller.update(&window, |state| state.set_source_language(request.language))?;
    persist(&app, &state)?;
    Ok(state)
}

#[tauri::command]
pub fn set_translation_engine<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    controller: State<'_, Controller>,
    request: TranslationEngineRequest,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    if let Some(sessions) = app.try_state::<CaptionSessionManager>() {
        sessions.stop_session();
    }
    let state = controller.update(&window, |state| {
        state.set_translation_engine(request.engine)
    })?;
    persist(&app, &state)?;
    // Availability depends on the chosen engine's files; re-probe so the
    // translate options enable or disable right away.
    if let (Some(sessions), Some(outputs)) = (
        app.try_state::<CaptionSessionManager>(),
        app.try_state::<Arc<OutputService>>(),
    ) {
        return sessions.refresh_resources(&controller, &outputs, &window);
    }
    Ok(state)
}

#[tauri::command]
pub fn set_ui_language<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    controller: State<'_, Controller>,
    request: UiLanguageRequest,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    let state = controller.update(&window, |state| state.set_ui_language(request.language))?;
    persist(&app, &state)?;
    Ok(state)
}

#[tauri::command]
pub fn start_capture<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    controller: State<'_, Controller>,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    start_live_capture(&app, &window, &controller)
}

#[tauri::command]
pub fn stop_capture<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    controller: State<'_, Controller>,
) -> Result<AppSnapshot, CommandError> {
    authorize(window.label())?;
    stop_live_capture(&app, &window, &controller)
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
    // O atalho de bloqueio não pode coincidir com o do ditado.
    if let Some(dictation) = app.try_state::<dictation_runtime::DictationState>() {
        let normalized = AppState::normalize_shortcut(&request.shortcut)?;
        if normalized.eq_ignore_ascii_case(&dictation.settings().shortcut) {
            return Err(CommandError::ShortcutUnavailable);
        }
    }
    let state = controller.change_shortcut(&app, &window, &request.shortcut)?;
    persist(&app, &state)?;
    Ok(state)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictationSnapshot {
    pub settings: DictationSettings,
    pub available: bool,
    pub phase: &'static str,
    pub inputs: DeviceSelection,
}

fn dictation_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    inputs: DeviceSelection,
) -> Result<DictationSnapshot, CommandError> {
    let state = app
        .try_state::<dictation_runtime::DictationState>()
        .ok_or(CommandError::StateUnavailable)?;
    Ok(DictationSnapshot {
        settings: state.settings(),
        available: state.available(),
        phase: state.phase_name(),
        inputs,
    })
}

#[tauri::command]
pub async fn get_dictation<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    inputs: State<'_, Arc<InputService>>,
) -> Result<DictationSnapshot, CommandError> {
    authorize(window.label())?;
    let inputs = Arc::clone(&inputs);
    let selection: DeviceSelection =
        tauri::async_runtime::spawn_blocking(move || inputs.selection())
            .await
            .map_err(|_| CommandError::StateUnavailable)?
            .map_err(CommandError::from)?;
    dictation_snapshot(&app, selection)
}

#[tauri::command]
pub async fn set_dictation<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    controller: State<'_, Controller>,
    inputs: State<'_, Arc<InputService>>,
    request: DictationSettings,
) -> Result<DictationSnapshot, CommandError> {
    authorize(window.label())?;
    let mut settings = request;
    settings.shortcut = crate::dictation::normalize_shortcut(&settings.shortcut)?;
    settings.validate(&controller.snapshot()?.shortcut)?;
    dictation_runtime::apply_settings(&app, settings.clone())?;
    if let Some(store) = app.try_state::<PreferenceStore>() {
        store
            .save_dictation(settings)
            .map_err(|_| CommandError::PreferencesUnavailable)?;
    }
    let inputs = Arc::clone(&inputs);
    let selection: DeviceSelection =
        tauri::async_runtime::spawn_blocking(move || inputs.selection())
            .await
            .map_err(|_| CommandError::StateUnavailable)?
            .map_err(CommandError::from)?;
    dictation_snapshot(&app, selection)
}

#[tauri::command]
pub async fn get_audio_outputs<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    service: State<'_, Arc<OutputService>>,
) -> Result<DeviceSelection, CommandError> {
    authorize(window.label())?;
    let service = Arc::clone(&service);
    let selection: DeviceSelection =
        tauri::async_runtime::spawn_blocking(move || service.selection())
            .await
            .map_err(|_| CommandError::StateUnavailable)?
            .map_err(CommandError::from)?;
    if let (Some(sessions), Some(controller), Some(outputs)) = (
        app.try_state::<CaptionSessionManager>(),
        app.try_state::<Controller>(),
        app.try_state::<Arc<OutputService>>(),
    ) {
        let _ = sessions.refresh_resources(&controller, &outputs, &window);
    }
    Ok(selection)
}

#[tauri::command]
pub async fn select_audio_output<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    service: State<'_, Arc<OutputService>>,
    request: OutputRequest,
) -> Result<DeviceSelection, CommandError> {
    authorize(window.label())?;
    let service = Arc::clone(&service);
    let app_for_save = app.clone();
    let selection: DeviceSelection = tauri::async_runtime::spawn_blocking(move || {
        let selection = service.select(&request.id)?;
        app_for_save
            .state::<PreferenceStore>()
            .save_output_id(request.id)
            .map_err(|_| CommandError::PreferencesUnavailable)?;
        Ok::<_, CommandError>(selection)
    })
    .await
    .map_err(|_| CommandError::StateUnavailable)??;
    if let (Some(sessions), Some(controller), Some(outputs)) = (
        app.try_state::<CaptionSessionManager>(),
        app.try_state::<Controller>(),
        app.try_state::<Arc<OutputService>>(),
    ) {
        let _ = stop_live_capture(&app, &window, &controller);
        let _ = sessions.refresh_resources(&controller, &outputs, &window);
    }
    Ok(selection)
}

/// The project page is the only URL the app ever opens. The command takes no
/// arguments, so the webview cannot steer the browser anywhere else.
pub const PROJECT_PAGE_URL: &str = "https://github.com/EduardoViniciusJ";

#[tauri::command]
pub fn open_project_page<R: Runtime>(window: WebviewWindow<R>) -> Result<(), CommandError> {
    authorize(window.label())?;
    crate::native::open_in_browser(PROJECT_PAGE_URL).map_err(|_| CommandError::BrowserUnavailable)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DownloadModelRequest {
    pub model: String,
}

/// One downloadable catalog model, with its state for the settings panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadState {
    pub model: &'static str,
    pub engine: TranslationEngine,
    pub installed: bool,
    pub downloadable: bool,
    pub downloading: bool,
    pub total_bytes: u64,
}

#[tauri::command]
pub fn get_model_downloads<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    downloads: State<'_, DownloadManager>,
) -> Result<Vec<ModelDownloadState>, CommandError> {
    authorize(window.label())?;
    let installed = app
        .try_state::<CaptionSessionManager>()
        .is_some_and(|sessions| sessions.translation_available(TranslationEngine::Riva));
    Ok(vec![ModelDownloadState {
        model: catalog::RIVA_TRANSLATE_Q4.id,
        engine: TranslationEngine::Riva,
        installed,
        downloadable: download::riva_source_configured(),
        downloading: downloads.active_model() == Some(catalog::RIVA_TRANSLATE_Q4.id),
        total_bytes: download::RIVA_DOWNLOAD_BYTES,
    }])
}

#[tauri::command]
pub fn download_model<R: Runtime>(
    window: WebviewWindow<R>,
    app: AppHandle<R>,
    downloads: State<'_, DownloadManager>,
    request: DownloadModelRequest,
) -> Result<(), CommandError> {
    authorize(window.label())?;
    let source = download::source_for(&request.model).ok_or(CommandError::DownloadUnavailable)?;
    let on_installed = {
        let app = app.clone();
        Box::new(move || refresh_after_install(&app))
    };
    downloads
        .start(
            source,
            crate::models::models_root(),
            Arc::new(window),
            on_installed,
        )
        .map_err(|_| CommandError::DownloadBusy)
}

#[tauri::command]
pub fn cancel_model_download<R: Runtime>(
    window: WebviewWindow<R>,
    downloads: State<'_, DownloadManager>,
) -> Result<(), CommandError> {
    authorize(window.label())?;
    downloads.cancel();
    Ok(())
}

/// A freshly installed model changes availability; re-probe so the translate
/// options enable without a restart.
fn refresh_after_install<R: Runtime>(app: &AppHandle<R>) {
    if let (Some(sessions), Some(outputs), Some(window)) = (
        app.try_state::<CaptionSessionManager>(),
        app.try_state::<Arc<OutputService>>(),
        app.get_webview_window("overlay"),
    ) {
        let controller = app.state::<Controller>();
        let _ = sessions.refresh_resources(&controller, &outputs, &window);
    }
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
            .update(&events, |state| {
                state.set_mode(CaptionMode::Translate(crate::domain::PT_BR))
            })
            .unwrap();
        let recorded = events.0.borrow();
        assert_eq!(recorded.len(), 1);
        assert_eq!(
            recorded[0].mode,
            CaptionMode::Translate(crate::domain::PT_BR)
        );
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
                    color: CaptionColor::WHITE,
                    partial_color: CaptionColor::DEFAULT_PARTIAL,
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
        let result = controller.update(&BrokenEvents, |state| {
            state.set_mode(CaptionMode::Translate(crate::domain::PT_BR))
        });
        assert_eq!(result.unwrap_err(), CommandError::EventUnavailable);
        assert_eq!(
            controller.snapshot().unwrap().mode,
            CaptionMode::Translate(crate::domain::PT_BR)
        );
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
        assert!(serde_json::from_str::<ModeRequest>(r#"{"mode":"en-en"}"#).is_err());
        assert!(serde_json::from_str::<TranslationEngineRequest>(r#"{"engine":"nllb"}"#).is_err());
        assert!(
            serde_json::from_str::<TranslationEngineRequest>(r#"{"engine":"opus","x":1}"#).is_err()
        );
        assert!(serde_json::from_str::<ModeRequest>(r#"{"mode":"auto","audio":[1,2]}"#).is_err());
        assert!(serde_json::from_str::<WindowRequest>(r#"{"mode":"hidden"}"#).is_err());
    }

    #[test]
    fn event_contracts_use_frontend_field_names() {
        let update = CaptionUpdate {
            session_id: 1,
            segment_id: 2,
            revision: 3,
            mode: CaptionMode::Translate(crate::domain::PT_BR),
            text: "<b>texto</b>".into(),
            is_final: true,
            language: Some("en-US".into()),
        };
        let json = serde_json::to_value(update).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"sessionId":1,"segmentId":2,"revision":3,"mode":"translate-pt-br","text":"<b>texto</b>","isFinal":true,"language":"en-US"})
        );
        let snapshot = serde_json::to_value(AppState::default().snapshot()).unwrap();
        assert_eq!(snapshot["windowMode"], "edit");
        assert_eq!(snapshot["translationEngine"], "opus");
        assert_eq!(snapshot["uiLanguage"], "pt-br");
        assert_eq!(snapshot["appearance"]["fontSize"], 32);
        assert_eq!(snapshot["appearance"]["color"], "#FFFFFF");
        assert_eq!(snapshot["appearance"]["partialColor"], "#B8B8B8");

        let download = serde_json::to_value(DownloadUpdate {
            model: "riva-translate-4b-instruct-v2-q4_k_m",
            phase: crate::models::download::DownloadPhase::Failed,
            received_bytes: 10,
            total_bytes: Some(20),
            reason: Some(crate::models::download::DownloadFailure::Network),
        })
        .unwrap();
        assert_eq!(
            download,
            serde_json::json!({"model":"riva-translate-4b-instruct-v2-q4_k_m","phase":"failed","receivedBytes":10,"totalBytes":20,"reason":"network"})
        );
        let state = serde_json::to_value(ModelDownloadState {
            model: "riva-translate-4b-instruct-v2-q4_k_m",
            engine: TranslationEngine::Riva,
            installed: false,
            downloadable: true,
            downloading: false,
            total_bytes: 2_759_302_368,
        })
        .unwrap();
        assert_eq!(
            state,
            serde_json::json!({"model":"riva-translate-4b-instruct-v2-q4_k_m","engine":"riva","installed":false,"downloadable":true,"downloading":false,"totalBytes":2_759_302_368_u64})
        );
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
