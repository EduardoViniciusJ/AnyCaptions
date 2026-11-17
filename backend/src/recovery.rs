use tauri::{
    App, AppHandle, Manager, Runtime, WindowEvent,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    domain::{CaptureState, UiLanguage, WindowMode},
    ipc::{CommandError, Controller, start_live_capture, stop_live_capture},
    preferences::PreferenceStore,
    transcription::session::CaptionSessionManager,
};

/// Tray menu texts, in the interface language.
struct TrayLabels {
    show: &'static str,
    capture: &'static str,
    capture_unavailable: &'static str,
    lock: &'static str,
    quit: &'static str,
}

fn tray_labels(language: UiLanguage) -> TrayLabels {
    match language.tag() {
        "pt-br" => TrayLabels {
            show: "Mostrar e editar",
            capture: "Iniciar/parar legendas",
            capture_unavailable: "Iniciar/parar (áudio e modelos indisponíveis)",
            lock: "Bloquear/desbloquear",
            quit: "Sair",
        },
        "es" => TrayLabels {
            show: "Mostrar y editar",
            capture: "Iniciar/detener subtítulos",
            capture_unavailable: "Iniciar/detener (audio y modelos no disponibles)",
            lock: "Bloquear/desbloquear",
            quit: "Salir",
        },
        "fr" => TrayLabels {
            show: "Afficher et modifier",
            capture: "Démarrer/arrêter les sous-titres",
            capture_unavailable: "Démarrer/arrêter (audio et modèles indisponibles)",
            lock: "Verrouiller/déverrouiller",
            quit: "Quitter",
        },
        "ru" => TrayLabels {
            show: "Показать и изменить",
            capture: "Запустить/остановить субтитры",
            capture_unavailable: "Запустить/остановить (аудио и модели недоступны)",
            lock: "Закрепить/открепить",
            quit: "Выход",
        },
        "zh-cn" => TrayLabels {
            show: "显示并编辑",
            capture: "开始/停止字幕",
            capture_unavailable: "开始/停止（音频和模型不可用）",
            lock: "锁定/解锁",
            quit: "退出",
        },
        _ => TrayLabels {
            show: "Show and edit",
            capture: "Start/stop captions",
            capture_unavailable: "Start/stop (audio and models unavailable)",
            lock: "Lock/unlock",
            quit: "Quit",
        },
    }
}

/// The tray items whose text follows the interface language.
struct TrayItems {
    show: MenuItem<tauri::Wry>,
    capture: MenuItem<tauri::Wry>,
    lock: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
    can_capture: bool,
}

/// Renames the tray menu after an interface language change.
pub(crate) fn relabel_tray<R: Runtime>(app: &AppHandle<R>, language: UiLanguage) {
    let Some(items) = app.try_state::<TrayItems>() else {
        return;
    };
    let labels = tray_labels(language);
    let _ = items.show.set_text(labels.show);
    let _ = items.capture.set_text(if items.can_capture {
        labels.capture
    } else {
        labels.capture_unavailable
    });
    let _ = items.lock.set_text(labels.lock);
    let _ = items.quit.set_text(labels.quit);
}

fn save_position<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("overlay")
        && let Ok(Some(geometry)) = crate::window::capture_geometry(&window)
    {
        let _ = app.state::<PreferenceStore>().save_geometry(geometry);
    }
}

pub(crate) fn edit<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = app
            .state::<Controller>()
            .change_window_mode(&window, WindowMode::Edit);
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// What the lock shortcut (and the tray's lock item) does next. A window the
/// user cannot see is brought back for editing, whatever its mode: before,
/// pressing the shortcut on a window closed to the tray only locked it, still
/// hidden, and it looked like the shortcut did nothing. A visible window
/// switches between locked and editable.
fn next_window_mode(current: WindowMode, on_screen: bool) -> WindowMode {
    if on_screen && current == WindowMode::Edit {
        WindowMode::Locked
    } else {
        WindowMode::Edit
    }
}

pub(crate) fn toggle<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("overlay")
        && let Ok(state) = app.state::<Controller>().snapshot()
    {
        let on_screen =
            window.is_visible().unwrap_or(true) && !window.is_minimized().unwrap_or(false);
        match next_window_mode(state.window_mode, on_screen) {
            WindowMode::Edit => edit(app),
            locked => {
                let _ = app
                    .state::<Controller>()
                    .change_window_mode(&window, locked);
            }
        }
    }
}

fn toggle_capture<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("overlay")
        && let Ok(state) = app.state::<Controller>().snapshot()
    {
        let controller = app.state::<Controller>();
        if state.capture == CaptureState::Stopped {
            let _ = start_live_capture(app, &window, &controller);
        } else {
            let _ = stop_live_capture(app, &window, &controller);
        }
    }
}

pub fn setup(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let can_capture = app
        .state::<Controller>()
        .snapshot()
        .is_ok_and(|s| s.resources.audio_capture && s.resources.transcription);
    let language = app
        .state::<Controller>()
        .snapshot()
        .map(|state| state.ui_language)
        .unwrap_or_default();
    let labels = tray_labels(language);
    let capture_label = if can_capture {
        labels.capture
    } else {
        labels.capture_unavailable
    };
    let show = MenuItem::with_id(app, "show", labels.show, true, None::<&str>)?;
    let capture = MenuItem::with_id(app, "capture", capture_label, can_capture, None::<&str>)?;
    let lock = MenuItem::with_id(app, "lock", labels.lock, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", labels.quit, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &capture, &lock, &quit])?;
    app.manage(TrayItems {
        show: show.clone(),
        capture: capture.clone(),
        lock: lock.clone(),
        quit: quit.clone(),
        can_capture,
    });
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or("Missing tray icon")?;
    TrayIconBuilder::new()
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => edit(app),
            "capture" => toggle_capture(app),
            "lock" => toggle(app),
            "quit" => {
                if app
                    .try_state::<crate::dictation::runtime::DictationState>()
                    .is_some()
                {
                    crate::dictation::runtime::shutdown(app);
                }
                if let Some(sessions) = app.try_state::<CaptionSessionManager>() {
                    sessions.stop_and_join();
                }
                save_position(app);
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    let window = app
        .get_webview_window("overlay")
        .ok_or("Missing overlay window")?;
    match app.state::<Controller>().enable_tray(&window) {
        Ok(_) | Err(CommandError::EventUnavailable) => {}
        Err(_) => return Err(std::io::Error::other("Cannot enable tray recovery").into()),
    }
    let shortcut = app
        .state::<Controller>()
        .snapshot()
        .map_err(|_| std::io::Error::other("Cannot read shortcut"))?
        .shortcut;
    let available = app
        .global_shortcut()
        .on_shortcut(shortcut.as_str(), |app, _, event| {
            if event.state == ShortcutState::Pressed {
                toggle(app);
            }
        })
        .is_ok();
    match app
        .state::<Controller>()
        .set_shortcut(shortcut, available, &window)
    {
        Ok(_) | Err(CommandError::EventUnavailable) => {}
        Err(_) => return Err(std::io::Error::other("Cannot update shortcut status").into()),
    }
    let close_window = window.clone();
    let close_app = app.handle().clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            save_position(&close_app);
            let _ = close_window.hide();
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lock_shortcut_brings_a_hidden_window_back_instead_of_locking_it() {
        // On screen: it switches between editable and locked.
        assert_eq!(next_window_mode(WindowMode::Edit, true), WindowMode::Locked);
        assert_eq!(next_window_mode(WindowMode::Locked, true), WindowMode::Edit);
        // Closed to the tray or minimized: it always comes back editable.
        assert_eq!(next_window_mode(WindowMode::Edit, false), WindowMode::Edit);
        assert_eq!(
            next_window_mode(WindowMode::Locked, false),
            WindowMode::Edit
        );
    }
}
