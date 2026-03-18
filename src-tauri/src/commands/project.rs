use serde::{Deserialize, Serialize};
use tauri::State;

use crate::error::AppError;
use crate::state::AppState;

/// Summary of an open project, returned to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub project_type: String,
    pub device_count: usize,
    pub device_ids: Vec<String>,
    pub total_sources: usize,
}

/// Get current project summary (derives from existing datasets)
#[tauri::command]
pub fn get_project_summary(
    state: State<AppState>,
) -> Result<ProjectSummary, AppError> {
    let datasets = state.datasets.read().map_err(|_| AppError::LockPoisoned)?;

    let device_ids: Vec<String> = datasets.keys().cloned().collect();
    let device_count = device_ids.len();

    Ok(ProjectSummary {
        project_type: if device_count <= 1 {
            "single_file".into()
        } else {
            "multi_file".into()
        },
        device_count,
        device_ids,
        total_sources: device_count, // 1 source per device for now
    })
}

/// Close current project -- clears all loaded datasets
#[tauri::command]
pub fn close_project(state: State<AppState>) -> Result<(), AppError> {
    let mut datasets = state.datasets.write().map_err(|_| AppError::LockPoisoned)?;
    datasets.clear();
    Ok(())
}
