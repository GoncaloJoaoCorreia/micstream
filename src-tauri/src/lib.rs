pub mod audio;
pub mod codec;
pub mod commands;
pub mod net;
pub mod protocol;
pub mod session;
pub mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::get_input_devices,
            commands::get_output_devices,
            commands::get_virtual_driver_status,
            commands::start_stream,
            commands::stop_stream,
            commands::start_listen,
            commands::stop_listen,
            commands::get_stream_status,
            commands::start_discovery,
            commands::stop_discovery,
            commands::get_app_config,
            commands::update_app_config,
            commands::set_target_jitter,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
