//! Tauri IPC コマンドモジュール群。
//!
//! フロントエンド (SvelteKit) から `invoke()` で呼び出される全コマンドを
//! 機能別サブモジュールに分割して管理する。
//!
//! - [`annotation`] — アノテーション (注釈) の保存・読み込み
//! - [`data`] — CSV データ読み込み・時系列チャンク取得
//! - [`device`] — デバイス単位のチャンク取得・統計計算
//! - [`export`] — データの CSV エクスポート
//! - [`project`] — プロジェクト管理 (開く・保存・閉じる)
//! - [`statistics`] — 統計量の計算

pub mod annotation;
pub mod data;
pub mod device;
pub mod export;
pub mod project;
pub mod statistics;
