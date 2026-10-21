fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Custom commands must participate in ACL checks, just like core commands.
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "get_state",
            "set_caption_mode",
            "start_capture",
            "stop_capture",
            "set_appearance",
            "set_window_mode",
            "set_shortcut",
            "get_audio_outputs",
            "select_audio_output",
        ]),
    ))?;
    Ok(())
}
