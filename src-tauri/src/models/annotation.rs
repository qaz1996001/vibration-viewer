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
