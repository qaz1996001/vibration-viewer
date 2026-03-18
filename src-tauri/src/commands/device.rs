//! デバイス単位のデータ取得 IPC コマンド。
//!
//! AIDPS プロジェクトでデバイス ID を指定してチャンク取得・統計計算を行う。
//! 内部的には `data::get_timeseries_chunk` / `statistics::compute_statistics` に
//! 委譲し、デバイス固有の channel フィルタリングを追加する。

use tauri::State;

use crate::error::AppError;
use crate::models::statistics::StatisticsReport;
use crate::models::vibration::TimeseriesChunk;
use crate::state::AppState;

/// 指定デバイスの時系列チャンクを取得する (チャンネルフィルタリング対応)。
///
/// `get_timeseries_chunk` に委譲した後、`channels` パラメータが指定されている
/// 場合は要求されたチャンネルのみを残してフィルタリングする。
///
/// # Parameters
/// - `device_id` — デバイス識別子 (データセットキーとして使用)
/// - `start_time` — 開始時刻 (epoch seconds, f64)
/// - `end_time` — 終了時刻 (epoch seconds, f64)
/// - `max_points` — ダウンサンプリング上限ポイント数
/// - `channels` — 取得するチャンネル名のリスト (`None` なら全チャンネル)
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Returns
/// [`TimeseriesChunk`] — フィルタ済み時系列チャンク
///
/// # Errors
/// - `AppError::DatasetNotFound` — 指定デバイスのデータが未ロード
/// - `AppError::LockPoisoned` — state ロック取得失敗
#[tauri::command]
pub fn get_device_chunk(
    device_id: String,
    start_time: f64,
    end_time: f64,
    max_points: usize,
    channels: Option<Vec<String>>,
    state: State<AppState>,
) -> Result<TimeseriesChunk, AppError> {
    let mut chunk = crate::commands::data::get_timeseries_chunk(
        device_id, start_time, end_time, max_points, state,
    )?;

    // Filter channels if requested
    if let Some(requested) = channels {
        chunk.channels.retain(|key, _| requested.contains(key));
    }

    Ok(chunk)
}

/// 指定デバイスの統計量を計算する。
///
/// `statistics::compute_statistics` に委譲し、`device_id` を
/// `dataset_id` として渡す。
///
/// # Parameters
/// - `device_id` — デバイス識別子
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Returns
/// [`StatisticsReport`] — 基本統計量・分布統計・形状統計
///
/// # Errors
/// - `AppError::DatasetNotFound` — 指定デバイスのデータが未ロード
/// - `AppError::LockPoisoned` — state ロック取得失敗
#[tauri::command]
pub fn get_device_stats(
    device_id: String,
    state: State<AppState>,
) -> Result<StatisticsReport, AppError> {
    crate::commands::statistics::compute_statistics(device_id, state)
}
