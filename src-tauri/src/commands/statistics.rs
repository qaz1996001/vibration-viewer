use tauri::State;

use crate::models::statistics::StatisticsReport;
use crate::services::stats_engine;
use crate::state::AppState;

#[tauri::command]
pub fn compute_statistics(
    dataset_id: String,
    state: State<AppState>,
) -> Result<StatisticsReport, String> {
    let datasets = state.datasets.lock().unwrap();
    let entry = datasets.get(&dataset_id).ok_or("Dataset not found")?;
    let df = &entry.dataframe;

    let axes = ["x", "y", "z", "amplitude"];

    let mut basic = Vec::new();
    let mut distribution = Vec::new();
    let mut shape = Vec::new();

    for axis in &axes {
        let col = df.column(axis).map_err(|e| e.to_string())?;
        let series = col.as_materialized_series();
        basic.push(stats_engine::compute_basic_stats(series, axis));
        distribution.push(stats_engine::compute_distribution_stats(series, axis));
        shape.push(stats_engine::compute_shape_stats(series, axis));
    }

    Ok(StatisticsReport {
        basic,
        distribution,
        shape,
    })
}
