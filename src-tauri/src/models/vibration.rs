use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMapping {
    pub time_column: String,
    pub data_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvPreview {
    pub file_path: String,
    pub columns: Vec<String>,
    pub row_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibrationDataset {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub total_points: usize,
    pub time_range: (f64, f64),
    pub column_mapping: ColumnMapping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeseriesChunk {
    pub time: Vec<f64>,
    pub channels: HashMap<String, Vec<f64>>,
    pub is_downsampled: bool,
    pub original_count: usize,
}
