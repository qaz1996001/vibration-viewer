//! `.vibproj` 项目文件读写服务。
//!
//! `.vibproj` 实质上是一个 ZIP 压缩包，内部结构如下:
//!
//! ```text
//! project.vibproj (ZIP)
//! ├── meta.json                  # 项目元数据、设备列表、传感器映射
//! ├── data/
//! │   ├── device1.parquet        # 设备 1 的时间序列数据 (Parquet 格式)
//! │   └── device2.parquet        # 设备 2 的时间序列数据
//! └── annotations.json           # 所有设备的标注数据 (可选)
//! ```
//!
//! 使用 Parquet 而非 CSV 存储数据，因为:
//! - 二进制列式格式，体积更小且读写更快
//! - 保留精确的 Float64 类型，避免重复解析

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;

use ::zip::write::SimpleFileOptions;
use ::zip::{ZipArchive, ZipWriter};
use polars::prelude::*;
use serde::{Deserialize, Serialize};

use crate::error::AppError;
use crate::models::annotation::Annotation;
use crate::models::project::{ProjectMetadata, ProjectType};
use crate::models::vibration::{ColumnMapping, VibrationDataset};
use crate::state::{DatasetEntry, ProjectContext};

/// `.vibproj` 内部 `meta.json` 的序列化结构。
///
/// 保存项目类型、元数据、设备列表等信息，加载时据此还原 [`ProjectContext`]。
/// `version` 字段用于未来格式升级时的向后兼容。
#[derive(Debug, Serialize, Deserialize)]
struct VibprojMeta {
    /// 格式版本号，当前为 1。未来结构变更时递增。
    version: u32,
    project_type: ProjectType,
    metadata: ProjectMetadata,
    sensor_mapping: HashMap<String, String>,
    /// 设备 ID 有序列表，与 `data/{id}.parquet` 条目一一对应。
    /// 排序确保保存/加载的确定性。
    device_ids: Vec<String>,
    /// 每个设备的 [`VibrationDataset`] 元数据，按设备 ID 索引。
    /// 加载时优先使用此处的存储值，避免从 Parquet 重新推导。
    dataset_metadata: HashMap<String, VibrationDataset>,
}

/// `.vibproj` 文件加载结果，包含完整的项目状态。
///
/// 调用方可将此结构中的数据直接注入 [`AppState`](crate::state::AppState)。
#[allow(dead_code)]
pub struct LoadedProject {
    pub project: ProjectContext,
    pub datasets: HashMap<String, DatasetEntry>,
    pub annotations: HashMap<String, Vec<Annotation>>,
}

/// 将当前项目状态保存为 `.vibproj` 文件。
///
/// 创建 Deflate 压缩的 ZIP 包，写入顺序:
/// 1. `meta.json` — 项目元数据与设备列表 (pretty-printed JSON)
/// 2. `data/{device_id}.parquet` — 每个设备的 DataFrame 序列化为 Parquet
/// 3. `annotations.json` — 所有标注按设备 ID 分组
///
/// 设备 ID 按字母序排列以确保输出确定性（方便 diff/测试）。
///
/// # Errors
/// - 文件创建失败: `AppError::Io`
/// - Parquet 序列化失败: `AppError::ProjectFile`
/// - ZIP 写入失败: `AppError::ProjectFile`
pub fn save_project(
    output_path: &Path,
    project: &ProjectContext,
    datasets: &HashMap<String, DatasetEntry>,
    annotations: &HashMap<String, Vec<Annotation>>,
) -> Result<(), AppError> {
    let file = std::fs::File::create(output_path)?;
    let mut zip = ZipWriter::new(file);
    let options =
        SimpleFileOptions::default().compression_method(::zip::CompressionMethod::Deflated);

    // Collect device IDs in deterministic order
    let mut device_ids: Vec<String> = datasets.keys().cloned().collect();
    device_ids.sort();

    // Build dataset metadata map
    let dataset_metadata: HashMap<String, VibrationDataset> = datasets
        .iter()
        .map(|(id, entry)| (id.clone(), entry.metadata.clone()))
        .collect();

    // 1. Write meta.json
    let meta = VibprojMeta {
        version: 1,
        project_type: project.project_type.clone(),
        metadata: project.metadata.clone(),
        sensor_mapping: project.sensor_mapping.clone(),
        device_ids: device_ids.clone(),
        dataset_metadata,
    };
    let meta_json = serde_json::to_string_pretty(&meta)?;
    zip.start_file("meta.json", options)
        .map_err(|e| AppError::ProjectFile(format!("Failed to write meta.json: {}", e)))?;
    zip.write_all(meta_json.as_bytes())?;

    // 2. Write per-device parquet files
    for device_id in &device_ids {
        if let Some(entry) = datasets.get(device_id) {
            let mut parquet_buf: Vec<u8> = Vec::new();
            {
                let cursor = Cursor::new(&mut parquet_buf);
                let mut df = entry.dataframe.clone();
                ParquetWriter::new(cursor).finish(&mut df).map_err(|e| {
                    AppError::ProjectFile(format!(
                        "Failed to write parquet for {}: {}",
                        device_id, e
                    ))
                })?;
            }
            let entry_path = format!("data/{}.parquet", device_id);
            zip.start_file(&entry_path, options).map_err(|e| {
                AppError::ProjectFile(format!("Failed to create ZIP entry {}: {}", entry_path, e))
            })?;
            zip.write_all(&parquet_buf)?;
        }
    }

    // 3. Write annotations.json
    let annotations_json = serde_json::to_string_pretty(annotations)?;
    zip.start_file("annotations.json", options)
        .map_err(|e| AppError::ProjectFile(format!("Failed to write annotations.json: {}", e)))?;
    zip.write_all(annotations_json.as_bytes())?;

    zip.finish()
        .map_err(|e| AppError::ProjectFile(format!("Failed to finalize ZIP: {}", e)))?;

    Ok(())
}

/// 加载 `.vibproj` 文件并还原完整的项目状态。
///
/// 从 ZIP 中按顺序读取:
/// 1. `meta.json` — 反序列化为 [`VibprojMeta`]
/// 2. `data/{id}.parquet` — 每个设备的 Parquet 文件读入 DataFrame，
///    优先使用 `meta.json` 中存储的元数据，若缺失则从 DataFrame 推导
/// 3. `annotations.json` — 可选，缺失时返回空 map
///
/// 注意: Parquet 文件需要完整缓冲到内存才能读取（`ZipFile` 不支持 `Seek`）。
///
/// # Errors
/// - 文件不存在 / 不是有效 ZIP: `AppError::ProjectFile`
/// - 缺少 `meta.json` 或所需的 Parquet 文件: `AppError::ProjectFile`
/// - Parquet 反序列化失败: `AppError::ProjectFile`
pub fn load_project(file_path: &Path) -> Result<LoadedProject, AppError> {
    let file = std::fs::File::open(file_path)
        .map_err(|e| AppError::ProjectFile(format!("Failed to open project file: {}", e)))?;
    let mut zip = ZipArchive::new(file).map_err(|e| {
        AppError::ProjectFile(format!("Invalid project file (not a valid ZIP): {}", e))
    })?;

    // 1. Read meta.json
    let meta: VibprojMeta = {
        let mut meta_file = zip.by_name("meta.json").map_err(|e| {
            AppError::ProjectFile(format!("meta.json not found in project file: {}", e))
        })?;
        let mut meta_buf = Vec::new();
        meta_file.read_to_end(&mut meta_buf)?;
        serde_json::from_slice(&meta_buf)?
    };

    // 2. Read per-device parquet files
    let mut datasets = HashMap::new();
    for device_id in &meta.device_ids {
        let entry_path = format!("data/{}.parquet", device_id);
        let mut parquet_file = zip.by_name(&entry_path).map_err(|e| {
            AppError::ProjectFile(format!(
                "Parquet file {} not found in project: {}",
                entry_path, e
            ))
        })?;

        // Buffer the entire parquet file since ZipFile doesn't implement Seek
        let mut parquet_buf = Vec::new();
        parquet_file.read_to_end(&mut parquet_buf)?;

        let cursor = Cursor::new(parquet_buf);
        let df = ParquetReader::new(cursor).finish().map_err(|e| {
            AppError::ProjectFile(format!("Failed to read parquet for {}: {}", device_id, e))
        })?;

        // Reconstruct metadata: use stored metadata if available, otherwise derive from DataFrame
        let metadata = if let Some(stored_meta) = meta.dataset_metadata.get(device_id) {
            stored_meta.clone()
        } else {
            derive_metadata_from_dataframe(device_id, &df)?
        };

        datasets.insert(
            device_id.clone(),
            DatasetEntry {
                metadata,
                dataframe: df,
            },
        );
    }

    // 3. Read annotations.json
    let annotations: HashMap<String, Vec<Annotation>> = match zip.by_name("annotations.json") {
        Ok(mut ann_file) => {
            let mut ann_buf: Vec<u8> = Vec::new();
            ann_file.read_to_end(&mut ann_buf)?;
            serde_json::from_slice(&ann_buf)?
        }
        Err(_) => HashMap::new(), // annotations.json is optional
    };

    // Build ProjectContext
    let project = ProjectContext {
        project_type: meta.project_type,
        metadata: meta.metadata,
        sensor_mapping: meta.sensor_mapping,
    };

    Ok(LoadedProject {
        project,
        datasets,
        annotations,
    })
}

/// 当 `meta.json` 中缺少某设备的元数据时，从 DataFrame 推导 [`VibrationDataset`]。
///
/// 推导逻辑:
/// - `total_points`: DataFrame 行数
/// - `time_range`: `"time"` 列的 min/max，列不存在时默认 `(0.0, 0.0)`
/// - `data_columns`: 除 `"time"` 外的所有列
/// - `file_path`: 设为空字符串（原始路径不可恢复）
///
/// 这是一个 fallback 路径，正常流程应使用 [`VibprojMeta::dataset_metadata`] 中的存储值。
fn derive_metadata_from_dataframe(
    device_id: &str,
    df: &DataFrame,
) -> Result<VibrationDataset, AppError> {
    let total_points = df.height();

    // Extract time range from the "time" column
    let time_range = if let Ok(time_col) = df.column("time") {
        let time_f64 = time_col
            .f64()
            .map_err(|e| AppError::ProjectFile(format!("time column is not Float64: {}", e)))?;
        let min = time_f64.min().unwrap_or(0.0);
        let max = time_f64.max().unwrap_or(0.0);
        (min, max)
    } else {
        (0.0, 0.0)
    };

    // Extract data columns (all columns except "time")
    let data_columns: Vec<String> = df
        .get_column_names()
        .iter()
        .filter(|name| name.as_str() != "time")
        .map(|name| name.to_string())
        .collect();

    Ok(VibrationDataset {
        id: device_id.to_string(),
        file_path: String::new(),
        file_name: format!("{}.parquet", device_id),
        total_points,
        time_range,
        column_mapping: ColumnMapping {
            time_column: "time".to_string(),
            data_columns,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create a test DataFrame with time + data columns
    fn make_test_dataframe() -> DataFrame {
        df!(
            "time" => &[1000.0_f64, 2000.0, 3000.0],
            "accel_x" => &[0.1_f64, 0.2, 0.3],
            "accel_y" => &[1.1_f64, 1.2, 1.3]
        )
        .unwrap()
    }

    fn make_test_dataset(id: &str) -> DatasetEntry {
        DatasetEntry {
            metadata: VibrationDataset {
                id: id.to_string(),
                file_path: format!("/data/{}.csv", id),
                file_name: format!("{}.csv", id),
                total_points: 3,
                time_range: (1000.0, 3000.0),
                column_mapping: ColumnMapping {
                    time_column: "time".to_string(),
                    data_columns: vec!["accel_x".to_string(), "accel_y".to_string()],
                },
            },
            dataframe: make_test_dataframe(),
        }
    }

    fn make_test_project() -> ProjectContext {
        ProjectContext {
            project_type: ProjectType::VibprojFile,
            metadata: ProjectMetadata {
                name: "Test Project".to_string(),
                created_at: "2026-03-18T10:00:00Z".to_string(),
                description: Some("A test project".to_string()),
            },
            sensor_mapping: HashMap::from([("sensor_a".to_string(), "device1".to_string())]),
        }
    }

    fn make_test_annotations() -> HashMap<String, Vec<Annotation>> {
        use crate::models::annotation::AnnotationType;
        let mut anns = HashMap::new();
        anns.insert(
            "device1".to_string(),
            vec![Annotation {
                id: "ann-001".to_string(),
                annotation_type: AnnotationType::Point {
                    time: 1500.0,
                    value: 0.15,
                    axis: "accel_x".to_string(),
                },
                label: "Test Peak".to_string(),
                color: "#ff0000".to_string(),
                label_offset_x: 0.0,
                label_offset_y: 0.0,
                created_at: "2026-03-18T10:00:00Z".to_string(),
            }],
        );
        anns
    }

    #[test]
    fn test_save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.vibproj");

        let project = make_test_project();
        let mut datasets = HashMap::new();
        datasets.insert("device1".to_string(), make_test_dataset("device1"));
        let annotations = make_test_annotations();

        // Save
        save_project(&file_path, &project, &datasets, &annotations).unwrap();
        assert!(file_path.exists());

        // Load
        let loaded = load_project(&file_path).unwrap();

        // Verify project context
        assert_eq!(loaded.project.project_type, ProjectType::VibprojFile);
        assert_eq!(loaded.project.metadata.name, "Test Project");
        assert_eq!(
            loaded.project.metadata.description,
            Some("A test project".to_string())
        );
        assert_eq!(loaded.project.sensor_mapping["sensor_a"], "device1");

        // Verify datasets
        assert_eq!(loaded.datasets.len(), 1);
        let ds = loaded.datasets.get("device1").unwrap();
        assert_eq!(ds.metadata.id, "device1");
        assert_eq!(ds.metadata.total_points, 3);
        assert!((ds.metadata.time_range.0 - 1000.0).abs() < 1e-6);
        assert!((ds.metadata.time_range.1 - 3000.0).abs() < 1e-6);
        assert_eq!(ds.dataframe.height(), 3);
        assert_eq!(ds.dataframe.width(), 3); // time, accel_x, accel_y

        // Verify data values
        let time = ds.dataframe.column("time").unwrap().f64().unwrap();
        assert!((time.get(0).unwrap() - 1000.0).abs() < 1e-6);
        assert!((time.get(2).unwrap() - 3000.0).abs() < 1e-6);
        let accel_x = ds.dataframe.column("accel_x").unwrap().f64().unwrap();
        assert!((accel_x.get(0).unwrap() - 0.1).abs() < 1e-6);

        // Verify annotations
        assert_eq!(loaded.annotations.len(), 1);
        let device_anns = loaded.annotations.get("device1").unwrap();
        assert_eq!(device_anns.len(), 1);
        assert_eq!(device_anns[0].id, "ann-001");
        assert_eq!(device_anns[0].label, "Test Peak");
    }

    #[test]
    fn test_save_empty_project() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("empty.vibproj");

        let project = ProjectContext::default();
        let datasets = HashMap::new();
        let annotations = HashMap::new();

        save_project(&file_path, &project, &datasets, &annotations).unwrap();
        assert!(file_path.exists());

        let loaded = load_project(&file_path).unwrap();
        assert!(loaded.datasets.is_empty());
        assert!(loaded.annotations.is_empty());
        assert_eq!(loaded.project.metadata.name, "Untitled Project");
    }

    #[test]
    fn test_load_invalid_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("not_a_zip.vibproj");

        // Write random bytes — not a valid ZIP
        std::fs::write(&file_path, b"this is not a zip file").unwrap();

        let result = load_project(&file_path);
        match result {
            Err(e) => {
                let err_msg = format!("{}", e);
                assert!(
                    err_msg.contains("Invalid project file") || err_msg.contains("not a valid ZIP"),
                    "Expected ZIP error, got: {}",
                    err_msg
                );
            }
            Ok(_) => panic!("Expected error for invalid file, got Ok"),
        }
    }

    #[test]
    fn test_meta_json_format() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("meta_check.vibproj");

        let project = make_test_project();
        let mut datasets = HashMap::new();
        datasets.insert("device1".to_string(), make_test_dataset("device1"));
        let annotations = HashMap::new();

        save_project(&file_path, &project, &datasets, &annotations).unwrap();

        // Open the ZIP and read meta.json directly
        let file = std::fs::File::open(&file_path).unwrap();
        let mut zip = ZipArchive::new(file).unwrap();
        let mut meta_file = zip.by_name("meta.json").unwrap();
        let mut meta_str = String::new();
        meta_file.read_to_string(&mut meta_str).unwrap();

        // Parse and verify structure
        let meta: serde_json::Value = serde_json::from_str(&meta_str).unwrap();
        assert_eq!(meta["version"], 1);
        assert_eq!(meta["project_type"], "VibprojFile");
        assert_eq!(meta["metadata"]["name"], "Test Project");
        assert_eq!(meta["metadata"]["description"], "A test project");
        assert_eq!(meta["device_ids"][0], "device1");
        assert!(meta["sensor_mapping"].is_object());
        assert_eq!(meta["sensor_mapping"]["sensor_a"], "device1");
        // Verify dataset_metadata is present
        assert!(meta["dataset_metadata"].is_object());
        assert!(meta["dataset_metadata"]["device1"].is_object());
        assert_eq!(meta["dataset_metadata"]["device1"]["id"], "device1");
    }

    #[test]
    fn test_save_and_load_multiple_devices() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("multi.vibproj");

        let project = make_test_project();
        let mut datasets = HashMap::new();
        datasets.insert("device1".to_string(), make_test_dataset("device1"));
        datasets.insert("device2".to_string(), make_test_dataset("device2"));
        let annotations = HashMap::new();

        save_project(&file_path, &project, &datasets, &annotations).unwrap();
        let loaded = load_project(&file_path).unwrap();

        assert_eq!(loaded.datasets.len(), 2);
        assert!(loaded.datasets.contains_key("device1"));
        assert!(loaded.datasets.contains_key("device2"));
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = load_project(Path::new("/nonexistent/path/file.vibproj"));
        assert!(result.is_err());
    }
}
