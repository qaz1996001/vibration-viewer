//! 振动时序数据模型。
//!
//! 包含 CSV 列映射、文件预览、数据集元信息以及前端分块传输用的
//! [`TimeseriesChunk`]。设计支持动态 CSV 列——用户通过
//! [`ColumnMapping`] 指定时间列和数据列，无硬编码 x/y/z。

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// CSV 列映射配置，由用户在 `ColumnMappingDialog` 中选定。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMapping {
    /// 时间列名（将被解析为 epoch seconds）
    pub time_column: String,
    /// 数据列名列表（如 `["accel_x", "accel_y", "accel_z"]`）
    pub data_columns: Vec<String>,
}

/// CSV 文件预览信息，用于两步加载流程的第一步。
///
/// 前端调用 `preview_csv_columns` 获取此结构后，弹出列映射对话框。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvPreview {
    /// CSV 文件绝对路径
    pub file_path: String,
    /// 文件中所有列名
    pub columns: Vec<String>,
    /// 总行数（不含表头）
    pub row_count: usize,
}

/// 已加载的振动数据集元信息（不含实际数据，DataFrame 存于 [`AppState`]）。
///
/// 通过 IPC 传递给前端，前端据此请求分块数据。
///
/// [`AppState`]: crate::state::AppState
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibrationDataset {
    /// 数据集唯一标识符
    pub id: String,
    /// CSV 文件绝对路径
    pub file_path: String,
    /// 文件名（不含目录）
    pub file_name: String,
    /// 数据总点数
    pub total_points: usize,
    /// 时间范围 `(min_epoch, max_epoch)`，单位为 epoch seconds
    pub time_range: (f64, f64),
    /// 用户选定的列映射配置
    pub column_mapping: ColumnMapping,
}

/// 时序数据分块，前端请求可视区间数据时返回。
///
/// `channels` 使用 [`IndexMap`] 以保持列的插入顺序（与用户选定的列顺序一致）。
/// 当数据量超过阈值时，后端执行 LTTB 降采样，`is_downsampled` 标记为 `true`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeseriesChunk {
    /// 时间戳序列（epoch seconds）
    pub time: Vec<f64>,
    /// channel 名称 → 数据值序列（与 `time` 等长，保持插入顺序）
    pub channels: IndexMap<String, Vec<f64>>,
    /// 是否经过 LTTB 降采样
    pub is_downsampled: bool,
    /// 降采样前的原始数据点数
    pub original_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_column_mapping_serialization() {
        let mapping = ColumnMapping {
            time_column: "datetime".into(),
            data_columns: vec!["accel_x".into(), "accel_y".into(), "accel_z".into()],
        };
        let json = serde_json::to_string(&mapping).unwrap();
        let deserialized: ColumnMapping = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.time_column, "datetime");
        assert_eq!(deserialized.data_columns.len(), 3);
        assert_eq!(deserialized.data_columns[2], "accel_z");
    }

    #[test]
    fn test_csv_preview_serialization() {
        let preview = CsvPreview {
            file_path: "/data/sensor.csv".into(),
            columns: vec!["time".into(), "x".into(), "y".into()],
            row_count: 50000,
        };
        let json = serde_json::to_string(&preview).unwrap();
        let deserialized: CsvPreview = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.file_path, "/data/sensor.csv");
        assert_eq!(deserialized.columns.len(), 3);
        assert_eq!(deserialized.row_count, 50000);
    }

    #[test]
    fn test_timeseries_chunk_serialization_with_indexmap_channels() {
        let mut channels = IndexMap::new();
        channels.insert("accel_x".to_string(), vec![0.1, 0.2, 0.3]);
        channels.insert("accel_y".to_string(), vec![1.1, 1.2, 1.3]);

        let chunk = TimeseriesChunk {
            time: vec![1000.0, 1001.0, 1002.0],
            channels,
            is_downsampled: false,
            original_count: 3,
        };

        let json = serde_json::to_string(&chunk).unwrap();
        let deserialized: TimeseriesChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.time.len(), 3);
        assert_eq!(deserialized.channels.len(), 2);
        assert_eq!(deserialized.channels["accel_x"], vec![0.1, 0.2, 0.3]);
        assert_eq!(deserialized.channels["accel_y"], vec![1.1, 1.2, 1.3]);
        assert!(!deserialized.is_downsampled);
        assert_eq!(deserialized.original_count, 3);
    }

    #[test]
    fn test_timeseries_chunk_channel_order_preserved() {
        // IndexMap preserves insertion order
        let mut channels = IndexMap::new();
        channels.insert("z".to_string(), vec![1.0]);
        channels.insert("a".to_string(), vec![2.0]);
        channels.insert("m".to_string(), vec![3.0]);

        let chunk = TimeseriesChunk {
            time: vec![0.0],
            channels,
            is_downsampled: true,
            original_count: 100,
        };

        let json = serde_json::to_string(&chunk).unwrap();
        let deserialized: TimeseriesChunk = serde_json::from_str(&json).unwrap();

        let keys: Vec<&String> = deserialized.channels.keys().collect();
        assert_eq!(keys, vec!["z", "a", "m"]);
    }

    #[test]
    fn test_vibration_dataset_serialization() {
        let dataset = VibrationDataset {
            id: "ds-001".into(),
            file_path: "/data/test.csv".into(),
            file_name: "test.csv".into(),
            total_points: 100000,
            time_range: (1700000000.0, 1700003600.0),
            column_mapping: ColumnMapping {
                time_column: "time".into(),
                data_columns: vec!["x".into()],
            },
        };
        let json = serde_json::to_string(&dataset).unwrap();
        let deserialized: VibrationDataset = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "ds-001");
        assert_eq!(deserialized.total_points, 100000);
        assert!((deserialized.time_range.0 - 1700000000.0).abs() < 1e-6);
    }

    #[test]
    fn test_column_mapping_empty_data_columns() {
        let mapping = ColumnMapping {
            time_column: "ts".into(),
            data_columns: vec![],
        };
        let json = serde_json::to_string(&mapping).unwrap();
        let deserialized: ColumnMapping = serde_json::from_str(&json).unwrap();
        assert!(deserialized.data_columns.is_empty());
    }
}
