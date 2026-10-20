pub mod domain;
pub mod ipc;

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
        ])
}

pub fn run() -> tauri::Result<()> {
    builder(tauri::Builder::default()).run(tauri::generate_context!())
}
