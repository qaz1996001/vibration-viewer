use polars::prelude::*;
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub fn export_data(
    dataset_id: String,
    output_path: String,
    start_time: Option<f64>,
    end_time: Option<f64>,
    state: State<AppState>,
) -> Result<String, AppError> {
    // Acquire lock, extract data, release lock before I/O
    let mut export_df = {
        let datasets = state.datasets.read().unwrap_or_else(|p| p.into_inner());
        let entry = datasets
            .get(&dataset_id)
            .ok_or_else(|| AppError::DatasetNotFound(dataset_id.clone()))?;
        let df = &entry.dataframe;

        match (start_time, end_time) {
            (Some(start), Some(end)) => df
                .clone()
                .lazy()
                .filter(
                    col("time")
                        .gt_eq(lit(start))
                        .and(col("time").lt_eq(lit(end))),
                )
                .collect()?,
            _ => df.clone(),
        }
    }; // Lock released here

    let mut file = std::fs::File::create(&output_path)?;
    CsvWriter::new(&mut file).finish(&mut export_df)?;

    Ok(output_path)
}
