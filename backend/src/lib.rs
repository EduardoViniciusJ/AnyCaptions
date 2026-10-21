pub mod audio;
pub mod domain;
pub mod ipc;
mod preferences;
mod recovery;
mod window;

pub fn builder<R: tauri::Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .manage(ipc::Controller::default())
        .invoke_handler(tauri::generate_handler![
            ipc::get_state,
            ipc::set_caption_mode,
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
            app.manage(std::sync::Arc::new(audio::devices::OutputService::new(
                Box::new(audio::devices::WasapiCatalog),
                saved.output_id.clone(),
            )));
            app.state::<ipc::Controller>()
                .restore_preferences(&saved)
                .map_err(|_| std::io::Error::other("Cannot restore preferences"))?;
            app.manage(store);
            window::setup(app)?;
            recovery::setup(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
}
