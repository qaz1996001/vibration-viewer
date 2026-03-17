use std::fs;
use std::path::PathBuf;

use crate::error::AppError;
use crate::models::annotation::*;

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
