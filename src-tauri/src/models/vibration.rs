use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnMapping {
    pub time_column: String,
    pub data_columns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvPreview {
    pub file_path: String,
    pub columns: Vec<String>,
    pub row_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibrationDataset {
    pub id: String,
    pub file_path: String,
    pub file_name: String,
    pub total_points: usize,
    pub time_range: (f64, f64),
    pub column_mapping: ColumnMapping,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeseriesChunk {
    pub time: Vec<f64>,
    pub channels: IndexMap<String, Vec<f64>>,
    pub is_downsampled: bool,
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
