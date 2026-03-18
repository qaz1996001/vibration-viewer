//! アノテーション (注釈) の保存・読み込み IPC コマンド。
//!
//! アノテーションは JSON sidecar ファイル (`{datafile}.vibann.json`) として
//! データファイルと同じディレクトリに保存される。ファイルパスの導出は
//! フロントエンド側で行い、Rust 側は指定されたパスに対して読み書きする。

use std::fs;
use std::path::PathBuf;

use crate::error::AppError;
use crate::models::annotation::*;

/// アノテーション一覧を JSON ファイルに保存する。
///
/// `AnnotationFile` (version + annotations) を pretty-print JSON で書き出す。
/// 既存ファイルは上書きされる。
///
/// # Parameters
/// - `annotation_path` — 保存先ファイルの絶対パス (`*.vibann.json`)
/// - `annotations` — 保存するアノテーション配列
///
/// # Errors
/// - JSON シリアライズエラー
/// - ファイル書き込みエラー (権限不足、ディスク容量等)
#[tauri::command]
pub fn save_annotations(
    annotation_path: String,
    annotations: Vec<Annotation>,
) -> Result<(), AppError> {
    let path = PathBuf::from(&annotation_path);

    let ann_file = AnnotationFile {
        version: 1,
        dataset_id: None,
        annotations,
    };

    let json = serde_json::to_string_pretty(&ann_file)?;
    fs::write(&path, json)?;

    Ok(())
}

/// JSON ファイルからアノテーション一覧を読み込む。
///
/// ファイルが存在しない場合は空の `Vec` を返す (初回読み込み時の正常動作)。
///
/// # Parameters
/// - `annotation_path` — 読み込み元ファイルの絶対パス (`*.vibann.json`)
///
/// # Returns
/// `Vec<Annotation>` — アノテーション配列 (ファイル未存在時は空)
///
/// # Errors
/// - ファイル読み込みエラー (権限不足等、存在しない場合を除く)
/// - JSON デシリアライズエラー (フォーマット不正)
#[tauri::command]
pub fn load_annotations(annotation_path: String) -> Result<Vec<Annotation>, AppError> {
    let path = PathBuf::from(&annotation_path);

    if !path.exists() {
        return Ok(Vec::new());
    }

    let json = fs::read_to_string(&path)?;
    let ann_file: AnnotationFile = serde_json::from_str(&json)?;

    Ok(ann_file.annotations)
}
