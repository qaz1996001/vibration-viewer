use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Project type determines how data was loaded
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ProjectType {
    /// Single CSV file opened directly
    SingleFile,
    /// AIDPS folder structure with history/ directory
    AidpsFolder,
    /// .vibproj ZIP project file
    VibprojFile,
}

/// A data source reference (CSV file, WAV file, etc.)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSource {
    pub file_path: String,
    pub file_name: String,
    pub source_type: DataSourceType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataSourceType {
    Csv,
    Wav,
}

/// Channel grouping schema (acceleration, extremes, VRMS, etc.)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelSchema {
    /// Groups of channel names, e.g. {"acceleration": ["x","y","z"], "vrms": ["vrms"]}
    pub groups: HashMap<String, Vec<String>>,
}

/// Per-device state: each device has its own data sources, annotations, and stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub sources: Vec<DataSource>,
    pub channel_schema: ChannelSchema,
}

/// Project metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadata {
    pub name: String,
    pub created_at: String,
    pub description: Option<String>,
}

/// The root project state sent to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub project_type: ProjectType,
    pub devices: Vec<DeviceInfo>,
    pub sensor_mapping: HashMap<String, String>,
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
                        make_csv_source("history_20260101.csv", "/aidps/device3/history/history_20260101.csv"),
                        make_csv_source("history_20260102.csv", "/aidps/device3/history/history_20260102.csv"),
                        make_csv_source("history_20260103.csv", "/aidps/device3/history/history_20260103.csv"),
                    ],
                    channel_schema: ChannelSchema {
                        groups: HashMap::from([
                            ("acceleration".into(), vec!["x".into(), "y".into(), "z".into()]),
                            ("vrms".into(), vec!["vrms".into()]),
                        ]),
                    },
                },
                DeviceInfo {
                    id: "device7".into(),
                    name: "Device 7".into(),
                    sources: vec![
                        make_csv_source("history_20260101.csv", "/aidps/device7/history/history_20260101.csv"),
                        make_csv_source("history_20260102.csv", "/aidps/device7/history/history_20260102.csv"),
                    ],
                    channel_schema: ChannelSchema {
                        groups: HashMap::from([
                            ("acceleration".into(), vec!["x".into(), "y".into(), "z".into()]),
                        ]),
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
