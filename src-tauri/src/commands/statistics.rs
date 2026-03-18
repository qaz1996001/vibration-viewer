use tauri::State;

use crate::error::AppError;
use crate::models::statistics::StatisticsReport;
use crate::services::stats_engine;
use crate::state::AppState;

#[tauri::command]
pub fn compute_statistics(
    dataset_id: String,
    state: State<AppState>,
) -> Result<StatisticsReport, AppError> {
    // Acquire read lock, clone out needed data, release lock before computation
    let (df_clone, data_columns) = {
        let datasets = state.datasets.read().map_err(|_| AppError::LockPoisoned)?;
        let entry = datasets
            .get(&dataset_id)
            .ok_or_else(|| AppError::DatasetNotFound(dataset_id.clone()))?;
        (entry.dataframe.clone(), entry.metadata.column_mapping.data_columns.clone())
    }; // Read lock released here

    // Heavy computation happens outside of lock scope
    let mut basic = Vec::new();
    let mut distribution = Vec::new();
    let mut shape = Vec::new();

    for col_name in &data_columns {
        let col = df_clone.column(col_name)?;
        let series = col.as_materialized_series();
        basic.push(stats_engine::compute_basic_stats(series, col_name)?);
        distribution.push(stats_engine::compute_distribution_stats(series, col_name)?);
        shape.push(stats_engine::compute_shape_stats(series, col_name)?);
    }

    Ok(StatisticsReport {
        basic,
        distribution,
        shape,
    })
}
