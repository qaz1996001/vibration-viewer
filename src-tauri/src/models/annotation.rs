use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AnnotationType {
    Point {
        time: f64,
        value: f64,
        axis: String,
    },
    Range {
        start_time: f64,
        end_time: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    pub annotation_type: AnnotationType,
    pub label: String,
    pub color: String,
    pub label_offset_x: f64,
    pub label_offset_y: f64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationFile {
    pub version: u32,
    pub dataset_id: Option<String>,
    pub annotations: Vec<Annotation>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_point_annotation(label: &str) -> Annotation {
        Annotation {
            id: "ann-001".into(),
            annotation_type: AnnotationType::Point {
                time: 1700000000.0,
                value: 0.5,
                axis: "accel_x".into(),
            },
            label: label.into(),
            color: "#ff0000".into(),
            label_offset_x: 10.0,
            label_offset_y: -5.0,
            created_at: "2024-01-15T10:30:00Z".into(),
        }
    }

    fn make_range_annotation(label: &str) -> Annotation {
        Annotation {
            id: "ann-002".into(),
            annotation_type: AnnotationType::Range {
                start_time: 1700000000.0,
                end_time: 1700000060.0,
            },
            label: label.into(),
            color: "#00ff00".into(),
            label_offset_x: 0.0,
            label_offset_y: 0.0,
            created_at: "2024-01-15T10:31:00Z".into(),
        }
    }

    #[test]
    fn test_annotation_point_roundtrip() {
        let ann = make_point_annotation("Test Peak");
        let json = serde_json::to_string(&ann).unwrap();
        let deserialized: Annotation = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "ann-001");
        assert_eq!(deserialized.label, "Test Peak");
        match &deserialized.annotation_type {
            AnnotationType::Point { time, value, axis } => {
                assert!((time - 1700000000.0).abs() < 1e-6);
                assert!((value - 0.5).abs() < 1e-6);
                assert_eq!(axis, "accel_x");
            }
            _ => panic!("Expected Point annotation type"),
        }
    }

    #[test]
    fn test_annotation_range_roundtrip() {
        let ann = make_range_annotation("Anomaly Window");
        let json = serde_json::to_string(&ann).unwrap();
        let deserialized: Annotation = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "ann-002");
        match &deserialized.annotation_type {
            AnnotationType::Range {
                start_time,
                end_time,
            } => {
                assert!((start_time - 1700000000.0).abs() < 1e-6);
                assert!((end_time - 1700000060.0).abs() < 1e-6);
            }
            _ => panic!("Expected Range annotation type"),
        }
    }

    #[test]
    fn test_annotation_unicode_labels() {
        let ann = make_point_annotation("異常振動峰值");
        let json = serde_json::to_string(&ann).unwrap();
        let deserialized: Annotation = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.label, "異常振動峰值");
    }

    #[test]
    fn test_annotation_file_roundtrip() {
        let ann_file = AnnotationFile {
            version: 1,
            dataset_id: Some("dataset-abc".into()),
            annotations: vec![
                make_point_annotation("Peak"),
                make_range_annotation("範圍標註"),
            ],
        };
        let json = serde_json::to_string_pretty(&ann_file).unwrap();
        let deserialized: AnnotationFile = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.version, 1);
        assert_eq!(deserialized.dataset_id, Some("dataset-abc".into()));
        assert_eq!(deserialized.annotations.len(), 2);
        assert_eq!(deserialized.annotations[0].label, "Peak");
        assert_eq!(deserialized.annotations[1].label, "範圍標註");
    }

    #[test]
    fn test_annotation_file_empty_annotations() {
        let ann_file = AnnotationFile {
            version: 1,
            dataset_id: None,
            annotations: vec![],
        };
        let json = serde_json::to_string(&ann_file).unwrap();
        let deserialized: AnnotationFile = serde_json::from_str(&json).unwrap();
        assert!(deserialized.annotations.is_empty());
        assert!(deserialized.dataset_id.is_none());
    }

    #[test]
    fn test_annotation_type_tagged_serialization() {
        // Verify the serde tag = "type" produces correct JSON structure
        let point = AnnotationType::Point {
            time: 1.0,
            value: 2.0,
            axis: "x".into(),
        };
        let json = serde_json::to_string(&point).unwrap();
        assert!(json.contains("\"type\":\"Point\""));
        assert!(json.contains("\"time\":1.0"));

        let range = AnnotationType::Range {
            start_time: 10.0,
            end_time: 20.0,
        };
        let json = serde_json::to_string(&range).unwrap();
        assert!(json.contains("\"type\":\"Range\""));
    }
}
