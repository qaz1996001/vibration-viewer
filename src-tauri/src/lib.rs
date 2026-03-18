//! Tauri 应用入口模块。
//!
//! 组装 Tauri Builder：注册插件（dialog、log）、注入 [`AppState`]、
//! 绑定所有 IPC command handler，然后启动事件循环。

mod commands;
mod error;
mod models;
mod services;
mod state;

use state::AppState;

/// 构建并启动 Tauri 应用。
///
/// 流程：
/// 1. 初始化 dialog 插件（文件选择对话框）
/// 2. debug 模式下启用 log 插件（`Info` 级别）
/// 3. 注入 [`AppState`] 全局状态
/// 4. 注册所有 IPC command handler
/// 5. 启动事件循环
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
            commands::data::load_device_data,
            commands::data::get_timeseries_chunk,
            commands::data::list_datasets,
            commands::statistics::compute_statistics,
            commands::annotation::save_annotations,
            commands::annotation::load_annotations,
            commands::export::export_data,
            // Phase 2: project & device commands
            commands::project::get_project_summary,
            commands::project::close_project,
            commands::project::open_aidps_project,
            commands::project::save_project_file,
            commands::project::load_project_file,
            commands::device::get_device_chunk,
            commands::device::get_device_stats,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
