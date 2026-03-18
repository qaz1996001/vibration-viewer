//! 项目与设备数据模型。
//!
//! 定义项目结构（单文件 / AIDPS 文件夹 / .vibproj 档案）、设备信息、
//! 数据源引用以及 channel 分组 schema，供前端 IPC 序列化使用。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 项目类型，决定数据的加载方式。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProjectType {
    /// 直接打开单个 CSV 文件
    SingleFile,
    /// AIDPS 文件夹结构（含 `history/` 目录）
    AidpsFolder,
    /// `.vibproj` ZIP 项目档案
    VibprojFile,
}

/// 数据源引用，指向一个 CSV 或 WAV 文件。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSource {
    /// 数据文件的绝对路径
    pub file_path: String,
    /// 文件名（不含目录）
    pub file_name: String,
    /// 数据源类型（CSV / WAV）
    pub source_type: DataSourceType,
}

/// 数据源文件格式。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataSourceType {
    /// CSV 时序数据文件
    Csv,
    /// WAV 音频/振动波形文件
    Wav,
}

/// Channel 分组 schema，将 channel 名按物理量归类。
///
/// 例如加速度组 `"acceleration": ["x","y","z"]`，VRMS 组 `"vrms": ["vrms"]`。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelSchema {
    /// 分组映射：group 名称 → 该组包含的 channel 名列表
    pub groups: HashMap<String, Vec<String>>,
}

/// 单个设备的完整信息，包括其数据源列表和 channel schema。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// 设备唯一标识符
    pub id: String,
    /// 设备显示名称
    pub name: String,
    /// 该设备关联的数据源列表
    pub sources: Vec<DataSource>,
    /// 该设备的 channel 分组定义
    pub channel_schema: ChannelSchema,
}

/// 项目元数据（名称、创建时间、描述）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    /// 项目名称
    pub name: String,
    /// 创建时间（ISO 8601 格式字符串）
    pub created_at: String,
    /// 可选的项目描述
    pub description: Option<String>,
}

/// 项目根信息，传递给前端的完整项目状态。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    /// 项目类型（单文件 / AIDPS / .vibproj）
    pub project_type: ProjectType,
    /// 项目中的所有设备
    pub devices: Vec<DeviceInfo>,
    /// sensor 名称 → device ID 的映射
    pub sensor_mapping: HashMap<String, String>,
    /// 项目元数据
    pub metadata: ProjectMetadata,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_csv_source(name: &str, path: &str) -> DataSource {
        DataSource {
            file_path: path.into(),
            file_name: name.into(),
            source_type: DataSourceType::Csv,
        }
    }

    fn make_wav_source(name: &str, path: &str) -> DataSource {
        DataSource {
            file_path: path.into(),
            file_name: name.into(),
            source_type: DataSourceType::Wav,
        }
    }

    fn make_device(id: &str, name: &str, sources: Vec<DataSource>) -> DeviceInfo {
        DeviceInfo {
            id: id.into(),
            name: name.into(),
            sources,
            channel_schema: ChannelSchema::default(),
        }
    }

    fn make_metadata(name: &str) -> ProjectMetadata {
        ProjectMetadata {
            name: name.into(),
            created_at: "2026-03-18T10:00:00Z".into(),
            description: None,
        }
    }

    #[test]
    fn test_project_info_serialization_roundtrip() {
        let project = ProjectInfo {
            project_type: ProjectType::AidpsFolder,
            devices: vec![make_device(
                "dev-1",
                "Device 1",
                vec![make_csv_source("data.csv", "/tmp/data.csv")],
            )],
            sensor_mapping: HashMap::from([("sensor_a".into(), "dev-1".into())]),
            metadata: make_metadata("Test Project"),
        };

        let json = serde_json::to_string_pretty(&project).unwrap();
        let deserialized: ProjectInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.project_type, ProjectType::AidpsFolder);
        assert_eq!(deserialized.devices.len(), 1);
        assert_eq!(deserialized.devices[0].id, "dev-1");
        assert_eq!(deserialized.sensor_mapping["sensor_a"], "dev-1");
        assert_eq!(deserialized.metadata.name, "Test Project");
        assert_eq!(deserialized.metadata.created_at, "2026-03-18T10:00:00Z");
        assert!(deserialized.metadata.description.is_none());
    }

    #[test]
    fn test_project_type_single_file_serialization() {
        let pt = ProjectType::SingleFile;
        let json = serde_json::to_string(&pt).unwrap();
        assert_eq!(json, "\"SingleFile\"");
        let deserialized: ProjectType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ProjectType::SingleFile);
    }

    #[test]
    fn test_project_type_aidps_folder_serialization() {
        let pt = ProjectType::AidpsFolder;
        let json = serde_json::to_string(&pt).unwrap();
        assert_eq!(json, "\"AidpsFolder\"");
        let deserialized: ProjectType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ProjectType::AidpsFolder);
    }

    #[test]
    fn test_project_type_vibproj_file_serialization() {
        let pt = ProjectType::VibprojFile;
        let json = serde_json::to_string(&pt).unwrap();
        assert_eq!(json, "\"VibprojFile\"");
        let deserialized: ProjectType = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, ProjectType::VibprojFile);
    }

    #[test]
    fn test_device_info_with_multiple_sources() {
        let device = DeviceInfo {
            id: "dev-multi".into(),
            name: "Multi-Source Device".into(),
            sources: vec![
                make_csv_source("accel.csv", "/data/accel.csv"),
                make_csv_source("temp.csv", "/data/temp.csv"),
                make_wav_source("audio.wav", "/data/audio.wav"),
            ],
            channel_schema: ChannelSchema {
                groups: HashMap::from([
                    (
                        "acceleration".into(),
                        vec!["x".into(), "y".into(), "z".into()],
                    ),
                    ("temperature".into(), vec!["temp_c".into()]),
                ]),
            },
        };

        let json = serde_json::to_string(&device).unwrap();
        let deserialized: DeviceInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, "dev-multi");
        assert_eq!(deserialized.sources.len(), 3);
        assert_eq!(deserialized.sources[0].source_type, DataSourceType::Csv);
        assert_eq!(deserialized.sources[2].source_type, DataSourceType::Wav);
        assert_eq!(deserialized.channel_schema.groups.len(), 2);
        assert_eq!(
            deserialized.channel_schema.groups["acceleration"],
            vec!["x", "y", "z"]
        );
    }

    #[test]
    fn test_channel_schema_default_is_empty() {
        let schema = ChannelSchema::default();
        assert!(schema.groups.is_empty());

        // Verify it serializes as empty map
        let json = serde_json::to_string(&schema).unwrap();
        assert_eq!(json, r#"{"groups":{}}"#);
    }

    #[test]
    fn test_single_file_degenerate_case() {
        // Single-file mode is a degenerate case of project mode:
        // one device with one CSV data source
        let project = ProjectInfo {
            project_type: ProjectType::SingleFile,
            devices: vec![DeviceInfo {
                id: "single".into(),
                name: "sensor_data.csv".into(),
                sources: vec![make_csv_source(
                    "sensor_data.csv",
                    "/home/user/sensor_data.csv",
                )],
                channel_schema: ChannelSchema {
                    groups: HashMap::from([(
                        "acceleration".into(),
                        vec!["accel_x".into(), "accel_y".into(), "accel_z".into()],
                    )]),
                },
            }],
            sensor_mapping: HashMap::new(),
            metadata: ProjectMetadata {
                name: "sensor_data.csv".into(),
                created_at: "2026-03-18T10:00:00Z".into(),
                description: None,
            },
        };

        let json = serde_json::to_string(&project).unwrap();
        let deserialized: ProjectInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.project_type, ProjectType::SingleFile);
        assert_eq!(deserialized.devices.len(), 1);
        assert_eq!(deserialized.devices[0].sources.len(), 1);
        assert_eq!(
            deserialized.devices[0].sources[0].source_type,
            DataSourceType::Csv
        );
    }

    #[test]
    fn test_aidps_case_multiple_devices_and_sources() {
        // AIDPS folder: multiple devices, each with multiple CSV data sources
        let project = ProjectInfo {
            project_type: ProjectType::AidpsFolder,
            devices: vec![
                DeviceInfo {
                    id: "device3".into(),
                    name: "Device 3".into(),
                    sources: vec![
                        make_csv_source(
                            "history_20260101.csv",
                            "/aidps/device3/history/history_20260101.csv",
                        ),
                        make_csv_source(
                            "history_20260102.csv",
                            "/aidps/device3/history/history_20260102.csv",
                        ),
                        make_csv_source(
                            "history_20260103.csv",
                            "/aidps/device3/history/history_20260103.csv",
                        ),
                    ],
                    channel_schema: ChannelSchema {
                        groups: HashMap::from([
                            (
                                "acceleration".into(),
                                vec!["x".into(), "y".into(), "z".into()],
                            ),
                            ("vrms".into(), vec!["vrms".into()]),
                        ]),
                    },
                },
                DeviceInfo {
                    id: "device7".into(),
                    name: "Device 7".into(),
                    sources: vec![
                        make_csv_source(
                            "history_20260101.csv",
                            "/aidps/device7/history/history_20260101.csv",
                        ),
                        make_csv_source(
                            "history_20260102.csv",
                            "/aidps/device7/history/history_20260102.csv",
                        ),
                    ],
                    channel_schema: ChannelSchema {
                        groups: HashMap::from([(
                            "acceleration".into(),
                            vec!["x".into(), "y".into(), "z".into()],
                        )]),
                    },
                },
            ],
            sensor_mapping: HashMap::from([
                ("sensor_north".into(), "device3".into()),
                ("sensor_south".into(), "device7".into()),
            ]),
            metadata: ProjectMetadata {
                name: "AIDPS Monitoring 2026-Q1".into(),
                created_at: "2026-03-18T10:00:00Z".into(),
                description: Some("Quarterly vibration monitoring data".into()),
            },
        };

        let json = serde_json::to_string(&project).unwrap();
        let deserialized: ProjectInfo = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.project_type, ProjectType::AidpsFolder);
        assert_eq!(deserialized.devices.len(), 2);
        assert_eq!(deserialized.devices[0].id, "device3");
        assert_eq!(deserialized.devices[0].sources.len(), 3);
        assert_eq!(deserialized.devices[1].id, "device7");
        assert_eq!(deserialized.devices[1].sources.len(), 2);
        assert_eq!(deserialized.sensor_mapping.len(), 2);
        assert_eq!(deserialized.sensor_mapping["sensor_north"], "device3");
        assert_eq!(
            deserialized.metadata.description,
            Some("Quarterly vibration monitoring data".into())
        );
    }
}
