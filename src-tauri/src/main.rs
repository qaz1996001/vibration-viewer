//! 振动数据查看器（Vibration Viewer）可执行入口。
//!
//! Release 构建时隐藏 Windows 控制台窗口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// 程序入口，委托给 [`app_lib::run`] 启动 Tauri 应用。
fn main() {
    app_lib::run();
}
