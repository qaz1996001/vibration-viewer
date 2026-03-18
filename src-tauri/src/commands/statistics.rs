//! 統計計算の IPC コマンド。
//!
//! データセットの全データ列に対して基本統計量 (mean, std, min, max)、
//! 分布統計 (histogram)、形状統計 (skewness, kurtosis) を一括計算し、
//! [`StatisticsReport`] として返す。

use tauri::State;

use crate::error::AppError;
use crate::models::statistics::StatisticsReport;
use crate::services::stats_engine;
use crate::state::AppState;

/// 指定データセットの全チャンネルに対して統計量を計算する。
///
/// データセットの `DataFrame` を読み取りロックで取得・clone した後、
/// ロック外で各チャンネルの統計計算を実行する。
///
/// # Parameters
/// - `dataset_id` — 対象データセットの ID
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Returns
/// [`StatisticsReport`] — 基本統計量・分布統計・形状統計の各チャンネル分
///
/// # Errors
/// - `AppError::DatasetNotFound` — 指定 ID のデータセットが未登録
/// - `AppError::LockPoisoned` — state ロック取得失敗
/// - Polars カラム抽出エラー
#[tauri::command]
pub fn compute_statistics(
    dataset_id: String,
    state: State<AppState>,
) -> Result<StatisticsReport, AppError> {
    // Acquire read lock, clone out needed data, release lock before computation
    let (df_clone, data_columns) = {
        let datasets = state.datasets.read().map_err(|_| AppError::LockPoisoned)?;
        let entry = datasets
            .get(&dataset_id)
            .ok_or_else(|| AppError::DatasetNotFound(dataset_id.clone()))?;
        (
            entry.dataframe.clone(),
            entry.metadata.column_mapping.data_columns.clone(),
        )
    }; // Read lock released here

    // Heavy computation happens outside of lock scope
    let mut basic = Vec::new();
    let mut distribution = Vec::new();
    let mut shape = Vec::new();

    for col_name in &data_columns {
        let col = df_clone.column(col_name)?;
        let series = col.as_materialized_series();
        basic.push(stats_engine::compute_basic_stats(series, col_name)?);
        distribution.push(stats_engine::compute_distribution_stats(series, col_name)?);
        shape.push(stats_engine::compute_shape_stats(series, col_name)?);
    }

    Ok(StatisticsReport {
        basic,
        distribution,
        shape,
    })
}
