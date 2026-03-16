use std::fs;
use std::path::PathBuf;

use crate::models::annotation::*;

#[tauri::command]
pub fn save_annotations(
    dataset_id: String,
    file_path: String,
    annotations: Vec<Annotation>,
) -> Result<(), String> {
    let ann_path = annotation_file_path(&file_path);

    let ann_file = AnnotationFile {
        version: 1,
        dataset_id,
        annotations,
    };

    let json =
        serde_json::to_string_pretty(&ann_file).map_err(|e| format!("Serialization failed: {}", e))?;

    fs::write(&ann_path, json).map_err(|e| format!("Write failed: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn load_annotations(file_path: String) -> Result<Vec<Annotation>, String> {
    let ann_path = annotation_file_path(&file_path);

    if !ann_path.exists() {
        return Ok(Vec::new());
    }

    let json = fs::read_to_string(&ann_path).map_err(|e| format!("Read failed: {}", e))?;

    let ann_file: AnnotationFile =
        serde_json::from_str(&json).map_err(|e| format!("JSON parse failed: {}", e))?;

    Ok(ann_file.annotations)
}

fn annotation_file_path(data_file_path: &str) -> PathBuf {
    let mut path = PathBuf::from(data_file_path);
    let filename = path.file_name().unwrap().to_string_lossy().to_string();
    path.set_file_name(format!("{}.vibann.json", filename));
    path
}
