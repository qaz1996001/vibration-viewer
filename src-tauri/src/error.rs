//! 应用统一错误类型。
//!
//! [`AppError`] 基于 `thiserror` 派生，覆盖 CSV 解析、数据查找、统计计算、
//! 标注操作、导出、IO、Polars、JSON 等场景。实现了 `Serialize` 以满足
//! Tauri IPC command 对 `Result` 错误类型的序列化要求。

use thiserror::Error;

/// 应用层统一错误枚举，所有 Tauri command 返回 `Result<T, AppError>`。
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum AppError {
    /// CSV 文件解析或读取失败
    #[error("CSV error: {0}")]
    Csv(String),

    /// 请求的通用资源未找到
    #[error("Data not found: {0}")]
    NotFound(String),

    /// 指定 ID 的 dataset 不存在于 `AppState` 中
    #[error("Dataset not found: {0}")]
    DatasetNotFound(String),

    /// CSV 中指定的列名不存在
    #[error("Column not found: {0}")]
    ColumnNotFound(String),

    /// 统计计算过程中的错误
    #[error("Statistics error: {0}")]
    Statistics(String),

    /// 标注操作（保存/加载）失败
    #[error("Annotation error: {0}")]
    Annotation(String),

    /// 数据导出失败
    #[error("Export error: {0}")]
    Export(String),

    /// `RwLock` 被 poison（某线程 panic 后持有锁）
    #[error("State lock poisoned")]
    LockPoisoned,

    /// 文件系统 IO 错误（自动从 `std::io::Error` 转换）
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Polars DataFrame 操作错误（自动从 `PolarsError` 转换）
    #[error("Polars error: {0}")]
    Polars(#[from] polars::error::PolarsError),

    /// JSON 序列化/反序列化错误（自动从 `serde_json::Error` 转换）
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// `.vibproj` 项目文件操作失败
    #[error("Project file error: {0}")]
    ProjectFile(String),

    /// 其他内部错误
    #[error("Internal error: {0}")]
    Internal(String),
}

/// 将 `AppError` 序列化为错误消息字符串。
///
/// Tauri IPC command 要求 `Result` 的 `Err` 类型实现 `Serialize`，
/// 这里直接将 `Display` 输出作为序列化结果。
impl serde::Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
