pub mod audio;
pub mod domain;
pub mod ipc;
pub mod models;
pub(crate) mod native;
pub mod pipeline;
mod preferences;
mod recovery;
pub mod transcription;
pub mod translation;
mod window;

pub fn builder<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .manage(ipc::Controller::default())
        .invoke_handler(tauri::generate_handler![
            ipc::get_state,
            ipc::set_caption_mode,
            ipc::set_ui_language,
            ipc::start_capture,
            ipc::stop_capture,
            ipc::set_appearance,
            ipc::set_window_mode,
            ipc::set_shortcut,
            ipc::get_audio_outputs,
            ipc::select_audio_output,
        ])
}

pub fn run() -> tauri::Result<()> {
    use tauri::Manager;

    builder(tauri::Builder::default())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let root = app.path().app_config_dir()?;
            let store = preferences::PreferenceStore::load(preferences::config_path(&root));
            let saved = store.snapshot()?;
            let outputs = std::sync::Arc::new(audio::devices::OutputService::new(
                Box::new(audio::devices::WasapiCatalog),
                saved.output_id.clone(),
            ));
            let sessions = transcription::session::CaptionSessionManager::default_native();
            app.state::<ipc::Controller>()
                .restore_preferences(&saved)
                .map_err(|_| std::io::Error::other("Cannot restore preferences"))?;
            app.manage(store);
            window::setup(app)?;
            if let Some(window) = app.get_webview_window("overlay") {
                let _ =
                    sessions.refresh_resources(&app.state::<ipc::Controller>(), &outputs, &window);
            }
            app.manage(outputs);
            app.manage(sessions);
            recovery::setup(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
}
