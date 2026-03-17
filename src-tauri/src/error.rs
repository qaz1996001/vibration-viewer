use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Dataset not found: {0}")]
    DatasetNotFound(String),

    #[error("Column not found: {0}")]
    ColumnNotFound(String),

    #[error("{0}")]
    Polars(#[from] polars::error::PolarsError),

    #[error("{0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Json(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    #[allow(dead_code)]
    Internal(String),
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}
