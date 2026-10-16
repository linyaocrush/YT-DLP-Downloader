//! Tauri application entry point: plugin registration, state and command wiring.

mod commands;
mod dto;
mod error;
mod state;
mod util;

/// Starts the desktop application.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::ytdlp_status,
            commands::detect_ytdlp,
            commands::ffmpeg_status,
            commands::default_download_dir,
            commands::list_cookie_files,
            commands::cookie_folder_status,
            commands::proxy_indicator,
            commands::proxy_summary,
            commands::parse_media,
            commands::get_media_info,
            commands::start_download,
            commands::cancel_job,
            commands::pick_file,
            commands::pick_folder,
            commands::open_url,
            commands::reveal_path,
            commands::notify,
            commands::set_taskbar_progress,
            commands::set_window_theme,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the tauri application");
}
