use polars::prelude::*;
use std::path::Path;

use crate::error::AppError;
use crate::models::vibration::{ColumnMapping, CsvPreview};

/// Maximum rows to read for column preview (keeps preview instant for large files).
const PREVIEW_ROW_LIMIT: usize = 100;

/// Preview CSV file: read only the first N rows for column detection.
/// Uses `with_n_rows` to avoid loading the entire file into memory,
/// making preview instant even for multi-GB files.
pub fn preview_csv(file_path: &str) -> Result<CsvPreview, AppError> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", file_path),
        )
        .into());
    }

    // Read only PREVIEW_ROW_LIMIT rows to detect columns quickly.
    // For total row count, we scan the full file lazily.
    let preview_df = CsvReadOptions::default()
        .with_n_rows(Some(PREVIEW_ROW_LIMIT))
        .try_into_reader_with_file_path(Some(path.into()))?
        .finish()?;

    let columns: Vec<String> = preview_df
        .get_column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Get total row count via lazy scan (streams without full materialization)
    let row_count = LazyCsvReader::new(file_path)
        .finish()?
        .select([len()])
        .collect()?
        .column("len")?
        .u32()?
        .get(0)
        .unwrap_or(0) as usize;

    Ok(CsvPreview {
        file_path: file_path.to_string(),
        columns,
        row_count,
    })
}

/// Convert a time column expression to Float64 epoch seconds based on its dtype.
///
/// - `String`: parse as datetime ("%Y-%m-%d %H:%M:%S"), cast to ms, divide by 1000
/// - `Datetime(_, _)`: cast to ms, divide by 1000
/// - Numeric/other: cast directly to Float64 (assumed already epoch seconds)
fn convert_time_column(time_col: &str, dtype: &DataType) -> Expr {
    match dtype {
        DataType::String => (col(time_col)
            .str()
            .to_datetime(
                Some(TimeUnit::Milliseconds),
                None,
                StrptimeOptions {
                    format: Some("%Y-%m-%d %H:%M:%S".into()),
                    ..Default::default()
                },
                lit("raise"),
            )
            .cast(DataType::Float64)
            / lit(1000.0))
        .alias("time"),
        DataType::Datetime(_, _) => {
            (col(time_col).cast(DataType::Float64) / lit(1000.0)).alias("time")
        }
        _ => col(time_col).cast(DataType::Float64).alias("time"),
    }
}

/// Read CSV with user-specified column mapping.
/// Parses the time column (string datetime / datetime / numeric) to epoch seconds.
/// Casts all data columns to Float64, preserving nulls (no fill_null).
pub fn read_csv_with_mapping(
    file_path: &str,
    mapping: &ColumnMapping,
) -> Result<DataFrame, AppError> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", file_path),
        )
        .into());
    }

    let df = CsvReadOptions::default()
        .try_into_reader_with_file_path(Some(path.into()))?
        .finish()?;

    // Validate columns exist
    let time_col = &mapping.time_column;
    if df.column(time_col).is_err() {
        return Err(AppError::ColumnNotFound(time_col.clone()));
    }
    for col_name in &mapping.data_columns {
        if df.column(col_name).is_err() {
            return Err(AppError::ColumnNotFound(col_name.clone()));
        }
    }

    let time_dtype = df.column(time_col)?.dtype().clone();
    let mut lazy = df.lazy();

    // Convert time column to Float64 (epoch seconds) via extracted helper
    lazy = lazy.with_column(convert_time_column(time_col, &time_dtype));

    // Cast data columns to Float64 (keep nulls as null — do NOT fill with 0.0).
    // NaN propagates through calculations instead of silently producing wrong results.
    let data_casts: Vec<Expr> = mapping
        .data_columns
        .iter()
        .map(|c| col(c).cast(DataType::Float64))
        .collect();
    if !data_casts.is_empty() {
        lazy = lazy.with_columns(data_casts);
    }

    // Drop rows where time is null (unparseable datetime)
    lazy = lazy.filter(col("time").is_not_null());

    // Select only the columns we need: time + data columns
    let mut select_cols = vec![col("time")];
    for c in &mapping.data_columns {
        select_cols.push(col(c));
    }
    lazy = lazy.select(select_cols);

    Ok(lazy.collect()?)
}
