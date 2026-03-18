//! プロジェクト管理の IPC コマンド群。
//!
//! プロジェクトの作成・オープン・保存・読み込み・クローズを担当する。
//! 単一ファイルモードと AIDPS フォルダモードの両方に対応し、
//! `.vibproj` ファイルによるプロジェクト永続化をサポートする。

use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

use tauri::State;

use crate::error::AppError;
use crate::models::project::*;
use crate::services::{project_file, project_scanner};
use crate::state::{AppState, ProjectContext};

/// 現在のプロジェクト概要を取得する。
///
/// ロード済みデータセットからデバイス一覧を動的に導出し、
/// `ProjectContext` のメタデータと合わせて [`ProjectInfo`] を構築する。
///
/// # Parameters
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Returns
/// [`ProjectInfo`] — プロジェクト種別、デバイス一覧、センサーマッピング、メタデータ
///
/// # Errors
/// - `AppError::LockPoisoned` — state ロック取得失敗
#[tauri::command]
pub fn get_project_summary(state: State<AppState>) -> Result<ProjectInfo, AppError> {
    let project = state.project.read().map_err(|_| AppError::LockPoisoned)?;
    let datasets = state.datasets.read().map_err(|_| AppError::LockPoisoned)?;

    let devices: Vec<DeviceInfo> = datasets
        .iter()
        .map(|(id, entry)| DeviceInfo {
            id: id.clone(),
            name: entry.metadata.file_name.clone(),
            sources: vec![DataSource {
                file_path: entry.metadata.file_path.clone(),
                file_name: entry.metadata.file_name.clone(),
                source_type: DataSourceType::Csv,
            }],
            channel_schema: ChannelSchema::default(),
        })
        .collect();

    Ok(ProjectInfo {
        project_type: project.project_type.clone(),
        devices,
        sensor_mapping: project.sensor_mapping.clone(),
        metadata: project.metadata.clone(),
    })
}

/// 現在のプロジェクトを閉じる — 全データセットをクリアし、プロジェクト状態をリセットする。
///
/// フロントエンドで「プロジェクトを閉じる」操作を行った際に呼ばれる。
/// `AppState` 内の `datasets` と `project` の両方を初期状態に戻す。
///
/// # Parameters
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Errors
/// - `AppError::LockPoisoned` — state ロック取得失敗
#[tauri::command]
pub fn close_project(state: State<AppState>) -> Result<(), AppError> {
    let mut datasets = state.datasets.write().map_err(|_| AppError::LockPoisoned)?;
    let mut project = state.project.write().map_err(|_| AppError::LockPoisoned)?;
    datasets.clear();
    *project = ProjectContext::default();
    Ok(())
}

/// AIDPS プロジェクトフォルダを開く — フォルダ構造をスキャンし、データ読み込みは行わない。
///
/// 以下の手順で処理する:
/// 1. `history/` サブディレクトリの存在を確認し、AIDPS フォルダ構造を検証
/// 2. デバイスサブディレクトリと CSV ファイルをスキャン
/// 3. プロジェクトコンテキスト (`project_type = AidpsFolder`) を `AppState` に設定
/// 4. 既存データセットをクリアして新規プロジェクト状態にする
///
/// データの実際の読み込みは、フロントエンドが `load_device_data` を個別に呼び出す。
///
/// # Parameters
/// - `folder_path` — AIDPS プロジェクトフォルダの絶対パス
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Returns
/// [`ProjectInfo`] — デバイス一覧・センサーマッピングを含むプロジェクト情報
///
/// # Errors
/// - `AppError::NotFound` — フォルダが存在しない、または `history/` ディレクトリがない
/// - `AppError::LockPoisoned` — state ロック取得失敗
/// - フォルダスキャン中の I/O エラー
#[tauri::command]
pub fn open_aidps_project(
    folder_path: String,
    state: State<AppState>,
) -> Result<ProjectInfo, AppError> {
    let path = Path::new(&folder_path);

    if !path.is_dir() {
        return Err(AppError::NotFound(format!(
            "Folder not found: {}",
            folder_path
        )));
    }

    if !project_scanner::is_aidps_folder(path) {
        return Err(AppError::NotFound(format!(
            "Not an AIDPS folder (no history/ directory): {}",
            folder_path
        )));
    }

    let scan_result = project_scanner::scan_aidps_folder(path)?;

    // Derive project name from folder name
    let project_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "AIDPS Project".to_string());

    // Build description including WAV availability
    let wav_note = if scan_result.wav_path.is_some() {
        " (WAV data available)"
    } else {
        ""
    };

    let metadata = ProjectMetadata {
        name: project_name,
        created_at: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| format!("{}", d.as_secs()))
            .unwrap_or_default(),
        description: Some(format!(
            "AIDPS project with {} devices{}",
            scan_result.devices.len(),
            wav_note
        )),
    };

    // Update project context in state
    let mut project = state.project.write().map_err(|_| AppError::LockPoisoned)?;
    *project = ProjectContext {
        project_type: ProjectType::AidpsFolder,
        metadata: metadata.clone(),
        sensor_mapping: scan_result.sensor_mapping.clone(),
    };

    // Clear any previously loaded datasets (new project = fresh state)
    let mut datasets = state.datasets.write().map_err(|_| AppError::LockPoisoned)?;
    datasets.clear();

    Ok(ProjectInfo {
        project_type: ProjectType::AidpsFolder,
        devices: scan_result.devices,
        sensor_mapping: scan_result.sensor_mapping,
        metadata,
    })
}

/// 現在のプロジェクト状態を `.vibproj` ファイルに保存する。
///
/// プロジェクトメタデータ、データセット情報を JSON 形式でシリアライズし、
/// 指定パスに書き出す。Annotation は sidecar ファイル方式のため、
/// 現時点では空の `HashMap` を渡している。
///
/// # Parameters
/// - `output_path` — 保存先 `.vibproj` ファイルの絶対パス
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Errors
/// - `AppError::LockPoisoned` — state ロック取得失敗
/// - ファイル書き込み・シリアライズエラー
#[tauri::command]
pub fn save_project_file(output_path: String, state: State<AppState>) -> Result<(), AppError> {
    let project = state.project.read().map_err(|_| AppError::LockPoisoned)?;
    let datasets = state.datasets.read().map_err(|_| AppError::LockPoisoned)?;

    // Annotations are stored as sidecar files; pass empty map for now
    let annotations = HashMap::new();

    project_file::save_project(Path::new(&output_path), &project, &datasets, &annotations)
}

/// `.vibproj` ファイルを読み込み、プロジェクト状態を復元する。
///
/// ファイルからデシリアライズしたデータセット群とプロジェクトコンテキストで
/// `AppState` を置き換えた後、[`get_project_summary`] を呼んで最新の
/// [`ProjectInfo`] を返す。
///
/// # Parameters
/// - `file_path` — `.vibproj` ファイルの絶対パス
/// - `state` — Tauri managed state ([`AppState`])
///
/// # Returns
/// [`ProjectInfo`] — 復元後のプロジェクト概要
///
/// # Errors
/// - ファイル読み込み・デシリアライズエラー
/// - `AppError::LockPoisoned` — state ロック取得失敗
#[tauri::command]
pub fn load_project_file(
    file_path: String,
    state: State<AppState>,
) -> Result<ProjectInfo, AppError> {
    let loaded = project_file::load_project(Path::new(&file_path))?;

    // Replace state with loaded data
    {
        let mut datasets = state.datasets.write().map_err(|_| AppError::LockPoisoned)?;
        *datasets = loaded.datasets;
    }
    {
        let mut project = state.project.write().map_err(|_| AppError::LockPoisoned)?;
        *project = loaded.project;
    }

    // Return the project summary
    get_project_summary(state)
}
