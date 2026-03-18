use polars::prelude::*;
use tauri::State;

use crate::error::AppError;
use crate::services::time_filter;
use crate::state::AppState;

#[tauri::command]
pub fn export_data(
    dataset_id: String,
    output_path: String,
    start_time: Option<f64>,
    end_time: Option<f64>,
    state: State<AppState>,
) -> Result<String, AppError> {
    // Acquire read lock, clone DataFrame, release lock before file I/O
    let df_clone = {
        let datasets = state.datasets.read().map_err(|_| AppError::LockPoisoned)?;
        let entry = datasets
            .get(&dataset_id)
            .ok_or_else(|| AppError::DatasetNotFound(dataset_id.clone()))?;
        entry.dataframe.clone()
    }; // Read lock released here

    // Apply time range filter using shared time_filter
    let mut export_df = match (start_time, end_time) {
        (Some(start), Some(end)) => time_filter::filter_time_range(&df_clone, "time", start, end)?,
        _ => df_clone,
    };

    // File I/O happens outside of lock scope
    let mut file = std::fs::File::create(&output_path)?;
    CsvWriter::new(&mut file).finish(&mut export_df)?;

    Ok(output_path)
}
