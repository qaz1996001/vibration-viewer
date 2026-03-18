//! 应用全局状态管理。
//!
//! [`AppState`] 通过 Tauri 的 `manage()` 注入，各 command handler 通过
//! `tauri::State<AppState>` 访问。内部使用 [`RwLock`] 实现线程安全的
//! 读写并发控制。

use std::collections::HashMap;
use std::sync::RwLock;

use polars::prelude::DataFrame;

use crate::models::project::{ProjectMetadata, ProjectType};
use crate::models::vibration::VibrationDataset;

/// 单个数据集的内存条目，包含元信息和实际 Polars DataFrame。
pub struct DatasetEntry {
    /// 数据集元信息（ID、路径、点数、时间范围、列映射）
    pub metadata: VibrationDataset,
    /// Polars DataFrame，持有完整的时序数据用于查询和降采样
    pub dataframe: DataFrame,
}

/// 项目级上下文，封装跨设备的项目元数据。
pub struct ProjectContext {
    /// 项目类型（单文件 / AIDPS / .vibproj）
    pub project_type: ProjectType,
    /// 项目元数据（名称、创建时间、描述）
    pub metadata: ProjectMetadata,
    /// sensor 名称 → device ID 的映射
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

/// 应用全局共享状态，由 Tauri `manage()` 注入。
///
/// 所有 Tauri command handler 通过 `tauri::State<AppState>` 获取引用，
/// 使用 `RwLock` 保证多 command 并发访问的线程安全。
pub struct AppState {
    /// 已加载的数据集映射：dataset ID → [`DatasetEntry`]
    pub datasets: RwLock<HashMap<String, DatasetEntry>>,
    /// 当前项目上下文
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
