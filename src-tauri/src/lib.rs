mod commands;
mod error;
mod models;
mod services;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::data::preview_csv_columns,
            commands::data::load_vibration_data,
            commands::data::get_timeseries_chunk,
            commands::statistics::compute_statistics,
            commands::annotation::save_annotations,
            commands::annotation::load_annotations,
            commands::export::export_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
