use polars::prelude::*;
use std::path::Path;

/// Read vibration CSV file into a Polars DataFrame.
/// Expected columns: time, x, y, z
/// Automatically computes amplitude = sqrt(x^2 + y^2 + z^2)
pub fn read_vibration_csv(file_path: &str) -> Result<DataFrame, String> {
    let path = Path::new(file_path);
    if !path.exists() {
        return Err(format!("File not found: {}", file_path));
    }

    let df = CsvReadOptions::default()
        .try_into_reader_with_file_path(Some(path.into()))
        .map_err(|e| format!("Cannot create CSV reader: {}", e))?
        .finish()
        .map_err(|e| format!("CSV parse failed: {}", e))?;

    let required = ["time", "x", "y", "z"];
    for col_name in &required {
        if df.column(col_name).is_err() {
            return Err(format!("Missing required column: {}", col_name));
        }
    }

    let df = df
        .lazy()
        .with_column(
            (col("x").pow(lit(2)) + col("y").pow(lit(2)) + col("z").pow(lit(2)))
                .sqrt()
                .alias("amplitude"),
        )
        .collect()
        .map_err(|e| format!("Failed to compute amplitude: {}", e))?;

    Ok(df)
}
