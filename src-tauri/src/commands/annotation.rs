use std::fs;
use std::path::PathBuf;

use crate::models::annotation::*;

#[tauri::command]
pub fn save_annotations(
    annotation_path: String,
    annotations: Vec<Annotation>,
) -> Result<(), String> {
    let path = PathBuf::from(&annotation_path);

    let ann_file = AnnotationFile {
        version: 1,
        dataset_id: None,
        annotations,
    };

    let json =
        serde_json::to_string_pretty(&ann_file).map_err(|e| format!("Serialization failed: {}", e))?;

    fs::write(&path, json).map_err(|e| format!("Write failed: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn load_annotations(annotation_path: String) -> Result<Vec<Annotation>, String> {
    let path = PathBuf::from(&annotation_path);

    if !path.exists() {
        return Ok(Vec::new());
    }

    let json = fs::read_to_string(&path).map_err(|e| format!("Read failed: {}", e))?;

    let ann_file: AnnotationFile =
        serde_json::from_str(&json).map_err(|e| format!("JSON parse failed: {}", e))?;

    Ok(ann_file.annotations)
}
