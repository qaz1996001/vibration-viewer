use tauri::State;

use crate::error::AppError;
use crate::models::statistics::StatisticsReport;
use crate::models::vibration::TimeseriesChunk;
use crate::state::AppState;

/// Get timeseries chunk for a specific device, with optional channel filtering
#[tauri::command]
pub fn get_device_chunk(
    device_id: String,
    start_time: f64,
    end_time: f64,
    max_points: usize,
    channels: Option<Vec<String>>,
    state: State<AppState>,
) -> Result<TimeseriesChunk, AppError> {
    let mut chunk = crate::commands::data::get_timeseries_chunk(
        device_id, start_time, end_time, max_points, state,
    )?;

    // Filter channels if requested
    if let Some(requested) = channels {
        chunk.channels.retain(|key, _| requested.contains(key));
    }

    Ok(chunk)
}

/// Get statistics for a specific device
#[tauri::command]
pub fn get_device_stats(
    device_id: String,
    state: State<AppState>,
) -> Result<StatisticsReport, AppError> {
    crate::commands::statistics::compute_statistics(device_id, state)
}
