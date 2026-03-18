//! AIDPS 项目文件夹扫描器。
//!
//! AIDPS (AI-based Diagnostic & Predictive System) 使用固定的目录结构存储振动数据:
//!
//! ```text
//! project_root/
//! ├── history/
//! │   ├── device1/           # 每个设备一个子目录
//! │   │   ├── dev1_001_20260101_120000.csv
//! │   │   └── dev1_002_20260102_120000.csv
//! │   └── device2/
//! │       └── dev2_001_20260101_120000.csv
//! └── wav/                   # 可选的音频目录
//! ```
//!
//! 本模块负责:
//! - 检测文件夹是否为 AIDPS 项目 ([`is_aidps_folder`])
//! - 扫描目录结构，收集设备与 CSV 文件列表 ([`scan_aidps_folder`])
//! - 从 CSV header 自动检测 channel schema ([`detect_channel_schema`])

use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::error::AppError;
use crate::models::project::{ChannelSchema, DataSource, DataSourceType, DeviceInfo};

/// AIDPS 文件夹扫描结果。
///
/// 包含检测到的设备列表、传感器映射关系、以及可选的 wav 目录路径。
#[derive(Debug)]
pub struct AidpsScanResult {
    pub devices: Vec<DeviceInfo>,
    pub sensor_mapping: HashMap<String, String>,
    pub wav_path: Option<String>,
}

/// AIDPS 已知的 channel 分组映射: 列名 -> 组名。
///
/// 返回 `None` 表示该列不属于任何已知组，将被归入 `"other"`。
fn aidps_column_group(col: &str) -> Option<&'static str> {
    match col {
        "x" | "y" | "z" => Some("acceleration"),
        "x_max" | "x_min" | "y_max" | "y_min" | "z_max" | "z_min" => Some("extremes"),
        "x_vrms" | "y_vrms" | "z_vrms" => Some("vrms"),
        _ => None,
    }
}

/// 从 CSV header 列名自动检测 channel schema（分组结构）。
///
/// 已知 AIDPS 分组:
/// - **acceleration**: `x`, `y`, `z`
/// - **extremes**: `x_max`, `x_min`, `y_max`, `y_min`, `z_max`, `z_min`
/// - **vrms**: `x_vrms`, `y_vrms`, `z_vrms`
///
/// `"time"` 列作为时间索引被排除。不属于任何已知组的列归入 `"other"`。
/// 若输入为空或仅含 `"time"` 列，返回空的 schema。
pub fn detect_channel_schema(columns: &[String]) -> ChannelSchema {
    let mut groups: HashMap<String, Vec<String>> = HashMap::new();

    for col in columns {
        // Skip the time column — it's the index, not a data channel
        if col == "time" {
            continue;
        }

        let group_name = aidps_column_group(col).unwrap_or("other");
        groups
            .entry(group_name.to_string())
            .or_default()
            .push(col.clone());
    }

    ChannelSchema { groups }
}

/// 检测给定路径是否为 AIDPS 项目文件夹。
///
/// 判据: 目录下存在 `history/` 子目录（AIDPS 项目的必要结构）。
pub fn is_aidps_folder(folder_path: &Path) -> bool {
    folder_path.join("history").is_dir()
}

/// 读取 CSV 文件的第一行并解析为列名列表。
///
/// 使用 `BufReader` 仅读取一行，不加载整个文件。
/// 列名按逗号分隔，前后空白会被 trim。
///
/// # Errors
/// - 文件为空: `AppError::Csv`
/// - 文件打开失败: `AppError::Io`
fn read_csv_header(csv_path: &Path) -> Result<Vec<String>, AppError> {
    let file = fs::File::open(csv_path)?;
    let mut reader = BufReader::new(file);
    let mut header_line = String::new();
    reader.read_line(&mut header_line)?;

    if header_line.is_empty() {
        return Err(AppError::Csv(format!(
            "Empty CSV file: {}",
            csv_path.display()
        )));
    }

    let columns: Vec<String> = header_line
        .trim()
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();

    Ok(columns)
}

/// 扫描 AIDPS 文件夹结构，构建完整的项目信息。
///
/// 扫描流程:
/// 1. 验证 `history/` 子目录存在
/// 2. 枚举 `history/` 下的设备子目录（按目录名排序，确保确定性）
/// 3. 每个设备子目录下收集 `.csv` 文件（按文件名排序，因为 AIDPS 文件名编码了日期时间）
/// 4. 从首个 CSV 的 header 自动检测 [`ChannelSchema`]，所有设备共享同一 schema
/// 5. 检查 `wav/` 目录是否存在
/// 6. 构建传感器映射（device ID -> device ID 的简单同名映射）
///
/// # Errors
/// - `history/` 不存在: `AppError::NotFound`
/// - `history/` 下无设备子目录: `AppError::NotFound`
/// - CSV header 读取失败: 传播 [`read_csv_header`] 的错误
pub fn scan_aidps_folder(folder_path: &Path) -> Result<AidpsScanResult, AppError> {
    // 1. Verify history/ directory exists
    let history_dir = folder_path.join("history");
    if !history_dir.is_dir() {
        return Err(AppError::NotFound(format!(
            "No history/ directory found in: {}",
            folder_path.display()
        )));
    }

    // 2. List device subdirectories in history/
    let mut device_dirs: Vec<_> = fs::read_dir(&history_dir)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .collect();

    // Sort by directory name for deterministic order
    device_dirs.sort_by_key(|e| e.file_name());

    if device_dirs.is_empty() {
        return Err(AppError::NotFound(format!(
            "No device subdirectories in: {}",
            history_dir.display()
        )));
    }

    // 3-4. For each device dir, collect CSV files and build DeviceInfo
    let mut devices = Vec::new();
    let mut first_schema: Option<ChannelSchema> = None;

    for device_entry in &device_dirs {
        let device_dir = device_entry.path();
        let device_name = device_entry.file_name().to_string_lossy().to_string();

        // Collect CSV files, sorted by filename (encodes date/time → chronological order)
        let mut csv_files: Vec<_> = fs::read_dir(&device_dir)?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|ext| ext.eq_ignore_ascii_case("csv"))
                    .unwrap_or(false)
            })
            .collect();

        csv_files.sort_by_key(|e| e.file_name());

        // 5. Auto-detect schema from first CSV header (once)
        if first_schema.is_none() {
            if let Some(first_csv) = csv_files.first() {
                let columns = read_csv_header(&first_csv.path())?;
                first_schema = Some(detect_channel_schema(&columns));
            }
        }

        let sources: Vec<DataSource> = csv_files
            .iter()
            .map(|entry| {
                let path = entry.path();
                DataSource {
                    file_path: path.to_string_lossy().to_string(),
                    file_name: entry.file_name().to_string_lossy().to_string(),
                    source_type: DataSourceType::Csv,
                }
            })
            .collect();

        let device = DeviceInfo {
            id: device_name.clone(),
            name: device_name,
            sources,
            channel_schema: first_schema.clone().unwrap_or_default(),
        };
        devices.push(device);
    }

    // 7. Check for wav/ directory
    let wav_dir = folder_path.join("wav");
    let wav_path = if wav_dir.is_dir() {
        Some(wav_dir.to_string_lossy().to_string())
    } else {
        None
    };

    // Build sensor mapping from device directory names
    // Convention: each device directory name maps to itself as sensor ID
    let sensor_mapping: HashMap<String, String> = devices
        .iter()
        .map(|d| (d.id.clone(), d.id.clone()))
        .collect();

    Ok(AidpsScanResult {
        devices,
        sensor_mapping,
        wav_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Create a minimal AIDPS folder structure for testing.
    fn create_aidps_structure(dir: &TempDir, devices: &[(&str, &[&str])], header: &str) {
        let history_dir = dir.path().join("history");
        fs::create_dir_all(&history_dir).unwrap();

        for (device_name, csv_files) in devices {
            let device_dir = history_dir.join(device_name);
            fs::create_dir_all(&device_dir).unwrap();

            for csv_file in *csv_files {
                let file_path = device_dir.join(csv_file);
                let mut f = fs::File::create(&file_path).unwrap();
                writeln!(f, "{}", header).unwrap();
                writeln!(f, "1.0,0.1,0.2,0.3,0.4,0.5,0.6,0.7,0.8,0.9,1.0,1.1,1.2").unwrap();
            }
        }
    }

    #[test]
    fn test_is_aidps_folder_valid() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("history")).unwrap();
        assert!(is_aidps_folder(dir.path()));
    }

    #[test]
    fn test_is_aidps_folder_invalid_no_history() {
        let dir = TempDir::new().unwrap();
        assert!(!is_aidps_folder(dir.path()));
    }

    #[test]
    fn test_is_aidps_folder_invalid_history_is_file() {
        let dir = TempDir::new().unwrap();
        // Create 'history' as a file, not a directory
        fs::File::create(dir.path().join("history")).unwrap();
        assert!(!is_aidps_folder(dir.path()));
    }

    #[test]
    fn test_scan_aidps_folder() {
        let dir = TempDir::new().unwrap();
        let header = "time,x,y,z,x_max,x_min,y_max,y_min,z_max,z_min,x_vrms,y_vrms,z_vrms";
        create_aidps_structure(
            &dir,
            &[
                (
                    "device1_1",
                    &[
                        "device1_1_001_20260101_120000.csv",
                        "device1_1_002_20260102_120000.csv",
                    ],
                ),
                ("device2", &["device2_001_20260101_120000.csv"]),
            ],
            header,
        );

        let result = scan_aidps_folder(dir.path()).unwrap();

        // Verify devices detected (sorted by name)
        assert_eq!(result.devices.len(), 2);
        assert_eq!(result.devices[0].id, "device1_1");
        assert_eq!(result.devices[1].id, "device2");

        // Verify CSV sources sorted by filename
        assert_eq!(result.devices[0].sources.len(), 2);
        assert_eq!(
            result.devices[0].sources[0].file_name,
            "device1_1_001_20260101_120000.csv"
        );
        assert_eq!(
            result.devices[0].sources[1].file_name,
            "device1_1_002_20260102_120000.csv"
        );

        // Verify single source for device2
        assert_eq!(result.devices[1].sources.len(), 1);

        // Verify channel schema was detected
        let schema = &result.devices[0].channel_schema;
        assert_eq!(schema.groups.len(), 3);
        assert!(schema.groups.contains_key("acceleration"));
        assert!(schema.groups.contains_key("extremes"));
        assert!(schema.groups.contains_key("vrms"));

        // Sensor mapping should map device IDs
        assert_eq!(result.sensor_mapping.len(), 2);
        assert_eq!(result.sensor_mapping["device1_1"], "device1_1");
        assert_eq!(result.sensor_mapping["device2"], "device2");

        // No wav directory
        assert!(result.wav_path.is_none());
    }

    #[test]
    fn test_scan_aidps_folder_with_wav() {
        let dir = TempDir::new().unwrap();
        let header = "time,x,y,z";
        create_aidps_structure(
            &dir,
            &[("device1", &["d1_001_20260101_000000.csv"])],
            header,
        );
        // Create wav/ directory
        fs::create_dir_all(dir.path().join("wav")).unwrap();

        let result = scan_aidps_folder(dir.path()).unwrap();
        assert!(result.wav_path.is_some());
    }

    #[test]
    fn test_scan_aidps_folder_no_history_dir() {
        let dir = TempDir::new().unwrap();
        let result = scan_aidps_folder(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("No history/ directory found"));
    }

    #[test]
    fn test_scan_aidps_folder_empty_history() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join("history")).unwrap();
        let result = scan_aidps_folder(dir.path());
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("No device subdirectories"));
    }

    #[test]
    fn test_detect_channel_schema() {
        let columns: Vec<String> = vec![
            "time", "x", "y", "z", "x_max", "x_min", "y_max", "y_min", "z_max", "z_min", "x_vrms",
            "y_vrms", "z_vrms",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let schema = detect_channel_schema(&columns);

        // Should have 3 groups: acceleration, extremes, vrms
        assert_eq!(schema.groups.len(), 3);

        let accel = &schema.groups["acceleration"];
        assert_eq!(accel, &["x", "y", "z"]);

        let extremes = &schema.groups["extremes"];
        assert_eq!(extremes.len(), 6);
        assert!(extremes.contains(&"x_max".to_string()));
        assert!(extremes.contains(&"z_min".to_string()));

        let vrms = &schema.groups["vrms"];
        assert_eq!(vrms.len(), 3);
        assert!(vrms.contains(&"x_vrms".to_string()));

        // "time" should NOT appear in any group
        for (_group_name, channels) in &schema.groups {
            assert!(!channels.contains(&"time".to_string()));
        }
    }

    #[test]
    fn test_detect_channel_schema_custom_columns() {
        let columns: Vec<String> = vec!["time", "temperature", "pressure", "humidity"]
            .into_iter()
            .map(String::from)
            .collect();

        let schema = detect_channel_schema(&columns);

        // All non-time columns should go into "other"
        assert_eq!(schema.groups.len(), 1);
        assert!(schema.groups.contains_key("other"));
        let other = &schema.groups["other"];
        assert_eq!(other.len(), 3);
        assert!(other.contains(&"temperature".to_string()));
        assert!(other.contains(&"pressure".to_string()));
        assert!(other.contains(&"humidity".to_string()));
    }

    #[test]
    fn test_detect_channel_schema_mixed_known_and_unknown() {
        let columns: Vec<String> = vec!["time", "x", "y", "z", "custom_sensor"]
            .into_iter()
            .map(String::from)
            .collect();

        let schema = detect_channel_schema(&columns);

        assert_eq!(schema.groups.len(), 2);
        assert_eq!(schema.groups["acceleration"], vec!["x", "y", "z"]);
        assert_eq!(schema.groups["other"], vec!["custom_sensor"]);
    }

    #[test]
    fn test_detect_channel_schema_empty_columns() {
        let columns: Vec<String> = vec![];
        let schema = detect_channel_schema(&columns);
        assert!(schema.groups.is_empty());
    }

    #[test]
    fn test_detect_channel_schema_only_time() {
        let columns: Vec<String> = vec!["time".into()];
        let schema = detect_channel_schema(&columns);
        assert!(schema.groups.is_empty());
    }

    #[test]
    fn test_scan_csv_files_sorted_chronologically() {
        let dir = TempDir::new().unwrap();
        let header = "time,x,y,z";
        // Create files with out-of-order filenames to test sorting
        let device_dir = dir.path().join("history").join("dev1");
        fs::create_dir_all(&device_dir).unwrap();

        for name in &[
            "dev1_003_20260103_120000.csv",
            "dev1_001_20260101_120000.csv",
            "dev1_002_20260102_120000.csv",
        ] {
            let mut f = fs::File::create(device_dir.join(name)).unwrap();
            writeln!(f, "{}", header).unwrap();
            writeln!(f, "1.0,0.1,0.2,0.3").unwrap();
        }

        let result = scan_aidps_folder(dir.path()).unwrap();
        let sources = &result.devices[0].sources;

        assert_eq!(sources.len(), 3);
        assert_eq!(sources[0].file_name, "dev1_001_20260101_120000.csv");
        assert_eq!(sources[1].file_name, "dev1_002_20260102_120000.csv");
        assert_eq!(sources[2].file_name, "dev1_003_20260103_120000.csv");
    }

    #[test]
    fn test_scan_ignores_non_csv_files() {
        let dir = TempDir::new().unwrap();
        let device_dir = dir.path().join("history").join("dev1");
        fs::create_dir_all(&device_dir).unwrap();

        // Create CSV and non-CSV files
        let mut csv_f = fs::File::create(device_dir.join("data_001.csv")).unwrap();
        writeln!(csv_f, "time,x,y,z").unwrap();
        writeln!(csv_f, "1.0,0.1,0.2,0.3").unwrap();

        fs::File::create(device_dir.join("readme.txt")).unwrap();
        fs::File::create(device_dir.join("notes.md")).unwrap();

        let result = scan_aidps_folder(dir.path()).unwrap();
        assert_eq!(result.devices[0].sources.len(), 1);
        assert_eq!(result.devices[0].sources[0].file_name, "data_001.csv");
    }

    #[test]
    fn test_read_csv_header() {
        let dir = TempDir::new().unwrap();
        let csv_path = dir.path().join("test.csv");
        let mut f = fs::File::create(&csv_path).unwrap();
        writeln!(f, "time,x,y,z").unwrap();
        writeln!(f, "1.0,0.1,0.2,0.3").unwrap();

        let columns = read_csv_header(&csv_path).unwrap();
        assert_eq!(columns, vec!["time", "x", "y", "z"]);
    }

    #[test]
    fn test_read_csv_header_empty_file() {
        let dir = TempDir::new().unwrap();
        let csv_path = dir.path().join("empty.csv");
        fs::File::create(&csv_path).unwrap();

        let result = read_csv_header(&csv_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_schema_shared_across_devices() {
        let dir = TempDir::new().unwrap();
        let header = "time,x,y,z,x_vrms,y_vrms,z_vrms";
        create_aidps_structure(
            &dir,
            &[("device_a", &["a_001.csv"]), ("device_b", &["b_001.csv"])],
            header,
        );

        let result = scan_aidps_folder(dir.path()).unwrap();

        // Both devices should share the same schema
        let schema_a = &result.devices[0].channel_schema;
        let schema_b = &result.devices[1].channel_schema;
        assert_eq!(schema_a.groups.len(), schema_b.groups.len());
        assert_eq!(schema_a.groups.len(), 2); // acceleration + vrms
    }
}
