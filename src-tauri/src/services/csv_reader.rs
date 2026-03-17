use polars::prelude::*;
use std::path::Path;

use crate::error::AppError;
use crate::models::vibration::{ColumnMapping, CsvPreview};

/// Preview CSV file: read headers and row count only.
pub fn preview_csv(file_path: &str) -> Result<CsvPreview, AppError> {
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

    let columns: Vec<String> = df
        .get_column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();
    let row_count = df.height();

    Ok(CsvPreview {
        file_path: file_path.to_string(),
        columns,
        row_count,
    })
}

/// Read CSV with user-specified column mapping.
/// Parses the time column (string datetime / datetime / numeric) to epoch seconds.
/// Casts all data columns to Float64.
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

    // Convert time column to Float64 (epoch seconds)
    match &time_dtype {
        DataType::String => {
            lazy = lazy.with_column(
                (col(time_col)
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
            );
        }
        DataType::Datetime(_, _) => {
            lazy = lazy.with_column(
                (col(time_col).cast(DataType::Float64) / lit(1000.0)).alias("time"),
            );
        }
        _ => {
            lazy = lazy.with_column(col(time_col).cast(DataType::Float64).alias("time"));
        }
    }

    // Cast data columns to Float64 (keep nulls as null — do NOT fill with 0.0)
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
