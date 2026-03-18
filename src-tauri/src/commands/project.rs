use tauri::State;

use crate::error::AppError;
use crate::models::project::*;
use crate::state::{AppState, ProjectContext};

/// Get current project summary (derives devices from loaded datasets + project context)
#[tauri::command]
pub fn get_project_summary(state: State<AppState>) -> Result<ProjectInfo, AppError> {
    let project = state.project.read().map_err(|_| AppError::LockPoisoned)?;
    let datasets = state.datasets.read().map_err(|_| AppError::LockPoisoned)?;

    let devices: Vec<DeviceInfo> = datasets
        .iter()
        .map(|(id, entry)| DeviceInfo {
            id: id.clone(),
            name: entry.metadata.file_name.clone(),
            sources: vec![DataSource {
                file_path: entry.metadata.file_path.clone(),
                file_name: entry.metadata.file_name.clone(),
                source_type: DataSourceType::Csv,
            }],
            channel_schema: ChannelSchema::default(),
        })
        .collect();

    Ok(ProjectInfo {
        project_type: project.project_type.clone(),
        devices,
        sensor_mapping: project.sensor_mapping.clone(),
        metadata: project.metadata.clone(),
    })
}

/// Close current project -- clears all loaded datasets and resets project context
#[tauri::command]
pub fn close_project(state: State<AppState>) -> Result<(), AppError> {
    let mut datasets = state.datasets.write().map_err(|_| AppError::LockPoisoned)?;
    let mut project = state.project.write().map_err(|_| AppError::LockPoisoned)?;
    datasets.clear();
    *project = ProjectContext::default();
    Ok(())
}
