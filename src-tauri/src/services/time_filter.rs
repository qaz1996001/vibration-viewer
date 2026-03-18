//! 时间范围过滤工具。
//!
//! 提供基于 Polars lazy API 的时间列范围过滤，供 chunk 请求和统计计算共用。
//! 使用 lazy API 可利用 Polars 的 SIMD 加速和谓词下推优化。

use polars::prelude::*;

/// 按时间范围过滤 DataFrame，保留 `time_col ∈ [start, end]` 的行。
///
/// 使用 Polars lazy API 执行过滤，底层利用 SIMD 加速。
/// 会 clone 输入 DataFrame（Polars clone 是 Arc-based 的，开销极低）。
///
/// # Parameters
/// - `df`: 输入 DataFrame
/// - `time_col`: 时间列名（通常为 `"time"`）
/// - `start`: 时间范围起点（闭区间，epoch seconds）
/// - `end`: 时间范围终点（闭区间，epoch seconds）
pub fn filter_time_range(
    df: &DataFrame,
    time_col: &str,
    start: f64,
    end: f64,
) -> Result<DataFrame, PolarsError> {
    df.clone()
        .lazy()
        .filter(
            col(time_col)
                .gt_eq(lit(start))
                .and(col(time_col).lt_eq(lit(end))),
        )
        .collect()
}
