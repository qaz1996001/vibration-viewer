use std::collections::HashMap;
use std::sync::RwLock;

use polars::prelude::DataFrame;

use crate::models::vibration::VibrationDataset;

pub struct DatasetEntry {
    pub metadata: VibrationDataset,
    pub dataframe: DataFrame,
}

pub struct AppState {
    pub datasets: RwLock<HashMap<String, DatasetEntry>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            datasets: RwLock::new(HashMap::new()),
        }
    }
}
