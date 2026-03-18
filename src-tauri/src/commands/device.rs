use tauri::State;

use crate::error::AppError;
use crate::models::statistics::StatisticsReport;
use crate::models::vibration::TimeseriesChunk;
use crate::state::AppState;

// Thin wrappers around existing functionality with device-oriented naming.
// In the full Phase 2 migration, these will replace the existing commands.

/// Get timeseries chunk for a specific device
#[tauri::command]
pub fn get_device_chunk(
    device_id: String,
    start_time: f64,
    end_time: f64,
    max_points: usize,
    _channels: Option<Vec<String>>,
    state: State<AppState>,
) -> Result<TimeseriesChunk, AppError> {
    // Delegate to existing get_timeseries_chunk logic.
    // The `_channels` parameter is reserved for future channel filtering.
    crate::commands::data::get_timeseries_chunk(device_id, start_time, end_time, max_points, state)
}

/// Get statistics for a specific device
#[tauri::command]
pub fn get_device_stats(
    device_id: String,
    state: State<AppState>,
) -> Result<StatisticsReport, AppError> {
    crate::commands::statistics::compute_statistics(device_id, state)
}
