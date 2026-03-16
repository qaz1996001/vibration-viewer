use std::collections::HashMap;
use std::sync::Mutex;

use polars::prelude::DataFrame;

use crate::models::vibration::VibrationDataset;

pub struct DatasetEntry {
    #[allow(dead_code)]
    pub metadata: VibrationDataset,
    pub dataframe: DataFrame,
}

pub struct AppState {
    pub datasets: Mutex<HashMap<String, DatasetEntry>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            datasets: Mutex::new(HashMap::new()),
        }
    }
}
