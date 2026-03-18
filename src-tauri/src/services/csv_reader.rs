use polars::prelude::*;
use std::path::Path;

use crate::error::AppError;
use crate::models::vibration::{ColumnMapping, CsvPreview};

/// Maximum rows to read for column preview (keeps preview instant for large files).
const PREVIEW_ROW_LIMIT: usize = 100;

/// Helper: write string content to a temp CSV file and return the path as String.
#[cfg(test)]
fn write_temp_csv(content: &str) -> (tempfile::NamedTempFile, String) {
    use std::io::Write;
    let mut tmp = tempfile::NamedTempFile::with_suffix(".csv").unwrap();
    tmp.write_all(content.as_bytes()).unwrap();
    tmp.flush().unwrap();
    let path = tmp.path().to_string_lossy().to_string();
    (tmp, path)
}

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

/// Read multiple CSV files and concatenate into a single DataFrame.
/// Used for AIDPS: merge all CSVs for one device into continuous timeseries.
///
/// Steps:
/// 1. Read each CSV using existing `read_csv_with_mapping` logic
/// 2. Vertical concat all DataFrames (polars `concat`)
/// 3. Sort by time column ascending
/// 4. Remove duplicate time entries (keep first)
///
/// Returns `(merged_dataframe, time_min, time_max)`.
pub fn concat_csvs(
    paths: &[String],
    mapping: &ColumnMapping,
) -> Result<(DataFrame, f64, f64), AppError> {
    if paths.is_empty() {
        return Err(AppError::Csv("No file paths provided for concatenation".into()));
    }

    let frames: Vec<DataFrame> = paths
        .iter()
        .map(|p| read_csv_with_mapping(p, mapping))
        .collect::<Result<Vec<_>, _>>()?;

    // Vertical concat all DataFrames (consume `frames` to avoid cloning)
    let lazy_frames: Vec<LazyFrame> = frames.into_iter().map(IntoLazy::lazy).collect();
    let combined = polars::lazy::dsl::concat(lazy_frames, UnionArgs::default())?
    // Remove duplicate time entries (keep first occurrence in concat order)
    .unique(Some(vec!["time".into()]), UniqueKeepStrategy::First)
    // Sort by time ascending
    .sort(
        ["time"],
        SortMultipleOptions::new().with_order_descending(false),
    )
    .collect()?;

    if combined.height() == 0 {
        return Err(AppError::Csv(
            "All rows were filtered out during concatenation".into(),
        ));
    }

    let time_col = combined.column("time")?.f64()?;
    let time_min = time_col.min().unwrap_or(0.0);
    let time_max = time_col.max().unwrap_or(0.0);

    Ok((combined, time_min, time_max))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── preview_csv tests ───

    #[test]
    fn test_preview_csv_returns_correct_columns_and_row_count() {
        let csv = "time,x,y,z\n1.0,0.1,0.2,0.3\n2.0,0.4,0.5,0.6\n3.0,0.7,0.8,0.9\n";
        let (_tmp, path) = write_temp_csv(csv);
        let preview = preview_csv(&path).unwrap();
        assert_eq!(preview.columns, vec!["time", "x", "y", "z"]);
        assert_eq!(preview.row_count, 3);
        assert_eq!(preview.file_path, path);
    }

    #[test]
    fn test_preview_csv_empty_csv_headers_only() {
        let csv = "time,accel_x,accel_y\n";
        let (_tmp, path) = write_temp_csv(csv);
        let preview = preview_csv(&path).unwrap();
        assert_eq!(preview.columns, vec!["time", "accel_x", "accel_y"]);
        assert_eq!(preview.row_count, 0);
    }

    #[test]
    fn test_preview_csv_single_row() {
        let csv = "ts,val\n42.0,99.9\n";
        let (_tmp, path) = write_temp_csv(csv);
        let preview = preview_csv(&path).unwrap();
        assert_eq!(preview.columns, vec!["ts", "val"]);
        assert_eq!(preview.row_count, 1);
    }

    #[test]
    fn test_preview_csv_file_not_found() {
        let result = preview_csv("/nonexistent/path/file.csv");
        assert!(result.is_err());
    }

    // ─── read_csv_with_mapping: numeric time (epoch seconds) ───

    #[test]
    fn test_read_csv_numeric_time_columns() {
        let csv = "ts,x,y\n1000.0,1.1,2.2\n2000.0,3.3,4.4\n3000.0,5.5,6.6\n";
        let (_tmp, path) = write_temp_csv(csv);
        let mapping = ColumnMapping {
            time_column: "ts".into(),
            data_columns: vec!["x".into(), "y".into()],
        };
        let df = read_csv_with_mapping(&path, &mapping).unwrap();
        assert_eq!(df.height(), 3);
        assert_eq!(df.width(), 3); // time, x, y

        let time = df.column("time").unwrap().f64().unwrap();
        assert!((time.get(0).unwrap() - 1000.0).abs() < 1e-6);
        assert!((time.get(2).unwrap() - 3000.0).abs() < 1e-6);
    }

    #[test]
    fn test_read_csv_integer_timestamps() {
        let csv = "epoch,val\n1700000000,10\n1700000001,20\n1700000002,30\n";
        let (_tmp, path) = write_temp_csv(csv);
        let mapping = ColumnMapping {
            time_column: "epoch".into(),
            data_columns: vec!["val".into()],
        };
        let df = read_csv_with_mapping(&path, &mapping).unwrap();
        assert_eq!(df.height(), 3);

        let time = df.column("time").unwrap().f64().unwrap();
        assert!((time.get(0).unwrap() - 1_700_000_000.0).abs() < 1e-6);
    }

    #[test]
    fn test_read_csv_datetime_string_parsed_to_epoch() {
        let csv = "datetime,sensor\n2024-01-15 10:30:00,1.5\n2024-01-15 10:30:01,2.5\n";
        let (_tmp, path) = write_temp_csv(csv);
        let mapping = ColumnMapping {
            time_column: "datetime".into(),
            data_columns: vec!["sensor".into()],
        };
        let df = read_csv_with_mapping(&path, &mapping).unwrap();
        assert_eq!(df.height(), 2);

        let time = df.column("time").unwrap().f64().unwrap();
        let t0 = time.get(0).unwrap();
        let t1 = time.get(1).unwrap();
        // Second row should be exactly 1 second later
        assert!((t1 - t0 - 1.0).abs() < 1e-3);
        // Epoch should be around 1705 billion ms / 1000 = ~1.7 billion
        assert!(t0 > 1_700_000_000.0);
    }

    #[test]
    fn test_read_csv_missing_column_returns_error() {
        let csv = "time,x\n1.0,2.0\n";
        let (_tmp, path) = write_temp_csv(csv);
        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["x".into(), "nonexistent".into()],
        };
        let result = read_csv_with_mapping(&path, &mapping);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_csv_missing_time_column_returns_error() {
        let csv = "a,b\n1.0,2.0\n";
        let (_tmp, path) = write_temp_csv(csv);
        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["a".into()],
        };
        let result = read_csv_with_mapping(&path, &mapping);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_csv_null_handling_preserves_nulls() {
        // CSV with a missing value — Polars reads it as null
        let csv = "time,x,y\n1.0,10.0,20.0\n2.0,,40.0\n3.0,30.0,\n";
        let (_tmp, path) = write_temp_csv(csv);
        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["x".into(), "y".into()],
        };
        let df = read_csv_with_mapping(&path, &mapping).unwrap();
        assert_eq!(df.height(), 3);

        let x = df.column("x").unwrap().f64().unwrap();
        // Row 1 (index 1) has null x
        assert!(x.get(1).is_none());
        // Row 0 and 2 have values
        assert!((x.get(0).unwrap() - 10.0).abs() < 1e-6);
        assert!((x.get(2).unwrap() - 30.0).abs() < 1e-6);

        let y = df.column("y").unwrap().f64().unwrap();
        assert!(y.get(2).is_none());
    }

    #[test]
    fn test_read_csv_single_row() {
        let csv = "time,sensor\n42.5,99.9\n";
        let (_tmp, path) = write_temp_csv(csv);
        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["sensor".into()],
        };
        let df = read_csv_with_mapping(&path, &mapping).unwrap();
        assert_eq!(df.height(), 1);
        let time = df.column("time").unwrap().f64().unwrap();
        assert!((time.get(0).unwrap() - 42.5).abs() < 1e-6);
    }

    #[test]
    fn test_read_csv_file_not_found() {
        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["x".into()],
        };
        let result = read_csv_with_mapping("/nonexistent/path.csv", &mapping);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_csv_selects_only_mapped_columns() {
        let csv = "time,a,b,c,d\n1.0,10,20,30,40\n2.0,11,21,31,41\n";
        let (_tmp, path) = write_temp_csv(csv);
        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["b".into(), "d".into()],
        };
        let df = read_csv_with_mapping(&path, &mapping).unwrap();
        // Should only have time, b, d
        assert_eq!(df.width(), 3);
        let col_names: Vec<String> = df
            .get_column_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(col_names.contains(&"time".to_string()));
        assert!(col_names.contains(&"b".to_string()));
        assert!(col_names.contains(&"d".to_string()));
        assert!(!col_names.contains(&"a".to_string()));
        assert!(!col_names.contains(&"c".to_string()));
    }

    // ─── concat_csvs tests ───

    #[test]
    fn test_concat_single_file() {
        let csv = "time,x,y\n2024-01-01 00:00:00,1.0,2.0\n2024-01-01 00:00:01,3.0,4.0\n";
        let (_tmp, path) = write_temp_csv(csv);
        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["x".into(), "y".into()],
        };
        let (df, time_min, time_max) = concat_csvs(&[path], &mapping).unwrap();
        assert_eq!(df.height(), 2);
        assert_eq!(df.width(), 3); // time, x, y
        assert!(time_min < time_max);
        assert!((time_max - time_min - 1.0).abs() < 1e-3);
    }

    #[test]
    fn test_concat_multiple_files() {
        // File 1: earlier times
        let csv1 = "time,x,y\n2024-01-01 00:00:00,1.0,2.0\n2024-01-01 00:00:01,3.0,4.0\n";
        let (_tmp1, path1) = write_temp_csv(csv1);

        // File 2: later times
        let csv2 = "time,x,y\n2024-01-01 00:00:02,5.0,6.0\n2024-01-01 00:00:03,7.0,8.0\n";
        let (_tmp2, path2) = write_temp_csv(csv2);

        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["x".into(), "y".into()],
        };
        let (df, time_min, time_max) = concat_csvs(&[path1, path2], &mapping).unwrap();
        assert_eq!(df.height(), 4);

        // Verify sorted by time
        let time = df.column("time").unwrap().f64().unwrap();
        for i in 1..df.height() {
            assert!(time.get(i).unwrap() >= time.get(i - 1).unwrap());
        }

        // Time range should span 3 seconds
        assert!((time_max - time_min - 3.0).abs() < 1e-3);
    }

    #[test]
    fn test_concat_removes_duplicate_times() {
        // Both files share the timestamp 00:00:01
        let csv1 = "time,x\n2024-01-01 00:00:00,1.0\n2024-01-01 00:00:01,2.0\n";
        let (_tmp1, path1) = write_temp_csv(csv1);

        let csv2 = "time,x\n2024-01-01 00:00:01,99.0\n2024-01-01 00:00:02,3.0\n";
        let (_tmp2, path2) = write_temp_csv(csv2);

        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["x".into()],
        };
        let (df, _, _) = concat_csvs(&[path1, path2], &mapping).unwrap();
        // 4 rows minus 1 duplicate = 3
        assert_eq!(df.height(), 3);

        // Verify time column has no duplicates
        let time = df.column("time").unwrap().f64().unwrap();
        let mut times: Vec<f64> = time.into_iter().flatten().collect();
        let orig_len = times.len();
        times.dedup();
        assert_eq!(times.len(), orig_len);

        // Verify "keep first" keeps file1's value (2.0), not file2's (99.0)
        let x = df.column("x").unwrap().f64().unwrap();
        // The middle row (index 1) corresponds to the duplicated timestamp
        // The value should be 2.0 from file1, not 99.0 from file2
        assert!((x.get(1).unwrap() - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_concat_preserves_all_channels() {
        let csv1 = "time,a,b,c\n2024-01-01 00:00:00,1.0,2.0,3.0\n";
        let (_tmp1, path1) = write_temp_csv(csv1);

        let csv2 = "time,a,b,c\n2024-01-01 00:00:01,4.0,5.0,6.0\n";
        let (_tmp2, path2) = write_temp_csv(csv2);

        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["a".into(), "b".into(), "c".into()],
        };
        let (df, _, _) = concat_csvs(&[path1, path2], &mapping).unwrap();
        assert_eq!(df.width(), 4); // time + a + b + c
        assert_eq!(df.height(), 2);

        let col_names: Vec<String> = df
            .get_column_names()
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert!(col_names.contains(&"time".to_string()));
        assert!(col_names.contains(&"a".to_string()));
        assert!(col_names.contains(&"b".to_string()));
        assert!(col_names.contains(&"c".to_string()));

        // Check values
        let a = df.column("a").unwrap().f64().unwrap();
        assert!((a.get(0).unwrap() - 1.0).abs() < 1e-6);
        assert!((a.get(1).unwrap() - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_concat_empty_paths_error() {
        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["x".into()],
        };
        let result = concat_csvs(&[], &mapping);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("No file paths"));
    }

    #[test]
    fn test_concat_mismatched_schemas_error() {
        // File1 has columns time,x,y — File2 has columns time,x,z (no "y")
        let csv1 = "time,x,y\n1.0,1.0,2.0\n";
        let (_tmp1, path1) = write_temp_csv(csv1);

        let csv2 = "time,x,z\n2.0,3.0,4.0\n";
        let (_tmp2, path2) = write_temp_csv(csv2);

        // Mapping requests "y" which exists in file1 but not file2
        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["x".into(), "y".into()],
        };
        let result = concat_csvs(&[path1, path2], &mapping);
        assert!(result.is_err());
    }

    #[test]
    fn test_concat_all_null_times_error() {
        // CSV with unparseable time values — all rows should be filtered out
        let csv = "time,x\nnot_a_date,1.0\nalso_bad,2.0\n";
        let (_tmp, path) = write_temp_csv(csv);

        let mapping = ColumnMapping {
            time_column: "time".into(),
            data_columns: vec!["x".into()],
        };
        let result = concat_csvs(&[path], &mapping);
        assert!(result.is_err());
    }
}
