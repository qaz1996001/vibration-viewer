use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum AppError {
    #[error("CSV error: {0}")]
    Csv(String),

    #[error("Data not found: {0}")]
    NotFound(String),

    #[error("Dataset not found: {0}")]
    DatasetNotFound(String),

    #[error("Column not found: {0}")]
    ColumnNotFound(String),

    #[error("Statistics error: {0}")]
    Statistics(String),

    #[error("Annotation error: {0}")]
    Annotation(String),

    #[error("Export error: {0}")]
    Export(String),

    #[error("State lock poisoned")]
    LockPoisoned,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Polars error: {0}")]
    Polars(#[from] polars::error::PolarsError),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

// Tauri commands need serializable errors
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
