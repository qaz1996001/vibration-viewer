//! CSV データ読み込み・時系列チャンク取得の IPC コマンド群。
//!
//! 単一ファイル読み込み (`load_vibration_data`)、AIDPS 複数ファイル結合
//! (`load_device_data`)、および LTTB ダウンサンプリング付きチャンク取得
//! (`get_timeseries_chunk`) を提供する。
//!
//! すべてのコマンドは [`AppState`] の `RwLock` を最小スコープで保持し、
//! 重い計算やファイル I/O はロック外で実行する。

use indexmap::IndexMap;
use polars::prelude::*;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::vibration::*;
use crate::services::{csv_reader, downsampling, time_filter};
use crate::state::{AppState, DatasetEntry};

/// CSV ファイルのカラム一覧をプレビュー取得する。
///
/// ファイルを部分的に読み込み、カラム名リストとサンプル行を返す。
/// フロントエンドの `ColumnMappingDialog` で時間列・データ列を選択させる
/// ための前段処理。
///
/// # Parameters
/// - `file_path` — CSV ファイルの絶対パス
///
/// # Returns
/// [`CsvPreview`] — カラム名一覧とサンプルデータ
///
/// # Errors
/// - ファイルが存在しない、または読み込めない場合
/// - CSV パースに失敗した場合
#[tauri::command]
pub fn preview_csv_columns(file_path: String) -> Result<CsvPreview, AppError> {
    csv_reader::preview_csv(&file_path)
}

/// 単一 CSV ファイルを読み込み、データセットとして `AppState` に登録する。
///
/// `ColumnMapping` に従って時間列・データ列を抽出し、Polars `DataFrame` として
/// メモリに保持する。登録後は UUID ベースの `dataset_id` で参照可能になる。
///
/// # Parameters
/// - `file_path` — CSV ファイルの絶対パス
/// - `column_mapping` — 時間列名・データ列名のマッピング (フロントエンドで選択済み)
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Returns
/// [`VibrationDataset`] — データセットメタデータ (id, ファイル名, 総ポイント数, 時間範囲)
///
/// # Errors
/// - CSV 読み込み・パース失敗
/// - `RwLock` poisoned (`AppError::LockPoisoned`)
#[tauri::command]
pub fn load_vibration_data(
    file_path: String,
    column_mapping: ColumnMapping,
    state: State<AppState>,
) -> Result<VibrationDataset, AppError> {
    // Perform file I/O outside of lock scope
    let df = csv_reader::read_csv_with_mapping(&file_path, &column_mapping)?;

    let time_col = df.column("time")?.f64()?;
    let total_points = df.height();
    let time_min = time_col.min().unwrap_or(0.0);
    let time_max = time_col.max().unwrap_or(0.0);

    let id = Uuid::new_v4().to_string();
    let file_name = Path::new(&file_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| file_path.clone());

    let metadata = VibrationDataset {
        id: id.clone(),
        file_path: file_path.clone(),
        file_name,
        total_points,
        time_range: (time_min, time_max),
        column_mapping,
    };

    // Acquire write lock only for the insert
    let mut datasets = state.datasets.write().map_err(|_| AppError::LockPoisoned)?;
    datasets.insert(
        id,
        DatasetEntry {
            metadata: metadata.clone(),
            dataframe: df,
        },
    );
    drop(datasets);

    // Update project context
    {
        let mut project_ctx = state.project.write().map_err(|_| AppError::LockPoisoned)?;
        if project_ctx.metadata.created_at.is_empty() {
            project_ctx.metadata.name = metadata.file_name.clone();
            project_ctx.metadata.created_at = "auto".into();
        }
        // Single-file mode with multi-file overlay; future: upgrade to AidpsFolder/VibprojFile
        project_ctx.project_type = crate::models::project::ProjectType::SingleFile;
    }

    Ok(metadata)
}

/// 指定時間範囲の時系列チャンクを取得する (LTTB ダウンサンプリング付き)。
///
/// フロントエンドの dataZoom イベントから呼ばれ、表示範囲に応じたデータを返す。
/// `max_points` を超える場合は LTTB (Largest-Triangle-Three-Buckets) アルゴリズムで
/// インデックスベースのダウンサンプリングを行い、全チャンネルに同一インデックスを適用する。
///
/// # Parameters
/// - `dataset_id` — 対象データセットの ID (UUID or device_id)
/// - `start_time` — 開始時刻 (epoch seconds, f64)
/// - `end_time` — 終了時刻 (epoch seconds, f64)
/// - `max_points` — フロントエンドの描画上限 (通常 50,000)
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Returns
/// [`TimeseriesChunk`] — 時刻配列、チャンネル別データ、ダウンサンプリング有無、元データ点数
///
/// # Errors
/// - `AppError::DatasetNotFound` — 指定 ID のデータセットが未登録
/// - `AppError::LockPoisoned` — state ロック取得失敗
/// - Polars カラム抽出エラー
#[tauri::command]
pub fn get_timeseries_chunk(
    dataset_id: String,
    start_time: f64,
    end_time: f64,
    max_points: usize,
    state: State<AppState>,
) -> Result<TimeseriesChunk, AppError> {
    // Acquire read lock, clone out needed data, then release lock before computation
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

    // Filter by time range using shared time_filter (SIMD-accelerated)
    let filtered = time_filter::filter_time_range(&df_clone, "time", start_time, end_time)?;
    let original_count = filtered.height();

    let time_raw = extract_f64_vec(&filtered, "time")?;

    if original_count > max_points {
        let representative = if !data_columns.is_empty() {
            extract_f64_vec(&filtered, &data_columns[0])?
        } else {
            vec![0.0; time_raw.len()]
        };
        let indices = downsampling::lttb_indices(&time_raw, &representative, max_points);

        let time: Vec<f64> = indices.iter().map(|&i| time_raw[i]).collect();
        let mut channels = IndexMap::new();
        for col_name in &data_columns {
            let raw = extract_f64_vec(&filtered, col_name)?;
            let sampled: Vec<f64> = indices.iter().map(|&i| raw[i]).collect();
            channels.insert(col_name.clone(), sampled);
        }

        Ok(TimeseriesChunk {
            time,
            channels,
            is_downsampled: true,
            original_count,
        })
    } else {
        let mut channels = IndexMap::new();
        for col_name in &data_columns {
            channels.insert(col_name.clone(), extract_f64_vec(&filtered, col_name)?);
        }

        Ok(TimeseriesChunk {
            time: time_raw,
            channels,
            is_downsampled: false,
            original_count,
        })
    }
}

/// AIDPS デバイスの複数 CSV ファイルを結合して単一データセットとして登録する。
///
/// 複数の CSV を時系列順に結合 (concat) し、重複タイムスタンプを除去した上で
/// `AppState` に格納する。`device_id` をキーとするため、同一デバイスで再呼び出し
/// すると既存エントリを上書きする (冪等)。
///
/// # Parameters
/// - `device_id` — デバイス識別子 (例: `"device3"`)。データセットキーとして使用
/// - `file_paths` — 結合対象の CSV ファイルパス一覧 (時系列順)
/// - `column_mapping` — 時間列・データ列のマッピング
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Returns
/// [`VibrationDataset`] — 結合後のデータセットメタデータ
///
/// # Errors
/// - CSV 読み込み・結合失敗
/// - `AppError::LockPoisoned` — state ロック取得失敗
#[tauri::command]
pub fn load_device_data(
    device_id: String,
    file_paths: Vec<String>,
    column_mapping: ColumnMapping,
    state: State<AppState>,
) -> Result<VibrationDataset, AppError> {
    // Perform file I/O outside of lock scope
    let (df, time_min, time_max) = csv_reader::concat_csvs(&file_paths, &column_mapping)?;

    let total_points = df.height();

    let file_name = format!("{} ({} files)", device_id, file_paths.len());

    let metadata = VibrationDataset {
        id: device_id.clone(),
        // TODO: file_path stores only first file; multi-file provenance not yet tracked
        file_path: file_paths.first().cloned().unwrap_or_default(),
        file_name,
        total_points,
        time_range: (time_min, time_max),
        column_mapping,
    };

    // Acquire write lock only for the insert
    let mut datasets = state.datasets.write().map_err(|_| AppError::LockPoisoned)?;
    datasets.insert(
        device_id,
        DatasetEntry {
            metadata: metadata.clone(),
            dataframe: df,
        },
    );
    drop(datasets);

    // Update project context for AIDPS multi-file mode
    {
        let mut project_ctx = state.project.write().map_err(|_| AppError::LockPoisoned)?;
        if project_ctx.metadata.created_at.is_empty() {
            project_ctx.metadata.name = metadata.file_name.clone();
            project_ctx.metadata.created_at = "auto".into();
        }
        project_ctx.project_type = crate::models::project::ProjectType::AidpsFolder;
    }

    Ok(metadata)
}

/// 現在ロード済みの全データセットのメタデータ一覧を返す。
///
/// プロジェクト読み込み後にフロントエンド側のストアを同期するために使用する。
///
/// # Parameters
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Returns
/// `Vec<VibrationDataset>` — 登録済みデータセットのメタデータ配列
///
/// # Errors
/// - `AppError::LockPoisoned` — state ロック取得失敗
#[tauri::command]
pub fn list_datasets(state: State<AppState>) -> Result<Vec<VibrationDataset>, AppError> {
    let datasets = state.datasets.read().map_err(|_| AppError::LockPoisoned)?;
    Ok(datasets.values().map(|entry| entry.metadata.clone()).collect())
}

/// Polars `DataFrame` から指定カラムを `Vec<f64>` として抽出するヘルパー。
///
/// `null` 値は `f64::NAN` に置換される。チャンク取得や統計計算で共通利用する。
///
/// # Parameters
/// - `df` — 対象の DataFrame
/// - `col_name` — 抽出するカラム名
///
/// # Errors
/// - カラムが存在しない、または `f64` にキャストできない場合
fn extract_f64_vec(df: &DataFrame, col_name: &str) -> Result<Vec<f64>, AppError> {
    Ok(df
        .column(col_name)?
        .f64()?
        .into_iter()
        .map(|opt| opt.unwrap_or(f64::NAN))
        .collect())
}
