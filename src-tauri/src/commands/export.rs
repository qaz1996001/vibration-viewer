//! データエクスポートの IPC コマンド。
//!
//! ロード済みデータセットを CSV ファイルとして書き出す。
//! オプションで時間範囲フィルタを適用できる。

use polars::prelude::*;
use tauri::State;

use crate::error::AppError;
use crate::services::time_filter;
use crate::state::AppState;

/// データセットを CSV ファイルとしてエクスポートする。
///
/// 指定された時間範囲 (`start_time` / `end_time`) がある場合はフィルタを適用し、
/// Polars `CsvWriter` で書き出す。時間範囲を省略すると全データをエクスポートする。
///
/// # Parameters
/// - `dataset_id` — エクスポート対象のデータセット ID
/// - `output_path` — 出力先 CSV ファイルの絶対パス
/// - `start_time` — 開始時刻 (epoch seconds, `None` ならフィルタなし)
/// - `end_time` — 終了時刻 (epoch seconds, `None` ならフィルタなし)
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Returns
/// `String` — エクスポート先のファイルパス (`output_path` と同一)
///
/// # Errors
/// - `AppError::DatasetNotFound` — 指定 ID のデータセットが未登録
/// - `AppError::LockPoisoned` — state ロック取得失敗
/// - ファイル作成・書き込みエラー
#[tauri::command]
pub fn export_data(
    dataset_id: String,
    output_path: String,
    start_time: Option<f64>,
    end_time: Option<f64>,
    state: State<AppState>,
) -> Result<String, AppError> {
    // Acquire read lock, clone DataFrame, release lock before file I/O
    let df_clone = {
        let datasets = state.datasets.read().map_err(|_| AppError::LockPoisoned)?;
        let entry = datasets
            .get(&dataset_id)
            .ok_or_else(|| AppError::DatasetNotFound(dataset_id.clone()))?;
        entry.dataframe.clone()
    }; // Read lock released here

    // Apply time range filter using shared time_filter
    let mut export_df = match (start_time, end_time) {
        (Some(start), Some(end)) => time_filter::filter_time_range(&df_clone, "time", start, end)?,
        _ => df_clone,
    };

    // File I/O happens outside of lock scope
    let mut file = std::fs::File::create(&output_path)?;
    CsvWriter::new(&mut file).finish(&mut export_df)?;

    Ok(output_path)
}
