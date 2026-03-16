use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibrationDataset {
    pub id: String,
    pub file_path: String,
    pub total_points: usize,
    pub time_range: (f64, f64),
    pub columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeseriesChunk {
    pub time: Vec<f64>,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
    pub amplitude: Vec<f64>,
    pub is_downsampled: bool,
    pub original_count: usize,
}
