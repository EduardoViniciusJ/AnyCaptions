use tauri::{
    App, AppHandle, Manager, Runtime, WindowEvent,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::{
    domain::WindowMode,
    ipc::{CommandError, Controller},
    preferences::PreferenceStore,
};

fn save_position<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("overlay")
        && let Ok(Some(geometry)) = crate::window::capture_geometry(&window)
    {
        let _ = app.state::<PreferenceStore>().save_geometry(geometry);
    }
}

fn edit<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("overlay") {
        let _ = app
            .state::<Controller>()
            .change_window_mode(&window, WindowMode::Edit);
    }
}

pub(crate) fn toggle<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window("overlay")
        && let Ok(state) = app.state::<Controller>().snapshot()
    {
        let next = if state.window_mode == WindowMode::Locked {
            WindowMode::Edit
        } else {
            WindowMode::Locked
        };
        let _ = app.state::<Controller>().change_window_mode(&window, next);
    }
}

pub fn setup(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "Mostrar e editar", true, None::<&str>)?;
    let capture = MenuItem::with_id(
        app,
        "capture",
        "Iniciar/parar (áudio e modelos indisponíveis)",
        false,
        None::<&str>,
    )?;
    let lock = MenuItem::with_id(app, "lock", "Bloquear/desbloquear", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &capture, &lock, &quit])?;
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
            "lock" => toggle(app),
            "quit" => {
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
