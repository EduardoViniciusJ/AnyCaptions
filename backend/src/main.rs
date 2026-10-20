#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if anycaptions_lib::run().is_err() {
        // Startup failures contain no captured audio, captions or filesystem paths.
        eprintln!("AnyCaptions: não foi possível iniciar a janela.");
        std::process::exit(1);
    }
}
