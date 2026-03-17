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
    let datasets = state.datasets.read().unwrap_or_else(|p| p.into_inner());
    let entry = datasets
        .get(&dataset_id)
        .ok_or_else(|| AppError::DatasetNotFound(dataset_id.clone()))?;
    let df = &entry.dataframe;
    let data_columns = &entry.metadata.column_mapping.data_columns;

    let mut basic = Vec::new();
    let mut distribution = Vec::new();
    let mut shape = Vec::new();

    for col_name in data_columns {
        let col = df.column(col_name)?;
        let series = col.as_materialized_series();
        basic.push(stats_engine::compute_basic_stats(series, col_name));
        distribution.push(stats_engine::compute_distribution_stats(series, col_name));
        shape.push(stats_engine::compute_shape_stats(series, col_name));
    }

    Ok(StatisticsReport {
        basic,
        distribution,
        shape,
    })
}
