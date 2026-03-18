use std::collections::HashMap;
use std::sync::RwLock;

use polars::prelude::DataFrame;

use crate::models::project::{ProjectMetadata, ProjectType};
use crate::models::vibration::VibrationDataset;

pub struct DatasetEntry {
    pub metadata: VibrationDataset,
    pub dataframe: DataFrame,
}

/// Project-level metadata wrapping the device datasets
pub struct ProjectContext {
    pub project_type: ProjectType,
    pub metadata: ProjectMetadata,
    pub sensor_mapping: HashMap<String, String>,
}

impl Default for ProjectContext {
    fn default() -> Self {
        Self {
            project_type: ProjectType::SingleFile,
            metadata: ProjectMetadata {
                name: "Untitled Project".into(),
                created_at: String::new(),
                description: None,
            },
            sensor_mapping: HashMap::new(),
        }
    }
}

pub struct AppState {
    /// Per-device data: each key is a device/dataset ID
    pub datasets: RwLock<HashMap<String, DatasetEntry>>,
    /// Project-level context
    pub project: RwLock<ProjectContext>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            datasets: RwLock::new(HashMap::new()),
            project: RwLock::new(ProjectContext::default()),
        }
    }
}
