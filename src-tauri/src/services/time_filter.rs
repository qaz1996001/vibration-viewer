use polars::prelude::*;

/// Filter a DataFrame to rows where the time column falls within [start, end].
/// Uses Polars lazy API for SIMD-accelerated filtering.
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
