use indexmap::IndexMap;
use std::path::Path;
use tauri::State;
use uuid::Uuid;
use polars::prelude::*;

use crate::error::AppError;
use crate::models::vibration::*;
use crate::services::{csv_reader, downsampling};
use crate::state::{AppState, DatasetEntry};

#[tauri::command]
pub fn preview_csv_columns(file_path: String) -> Result<CsvPreview, AppError> {
    csv_reader::preview_csv(&file_path)
}

#[tauri::command]
pub fn load_vibration_data(
    file_path: String,
    column_mapping: ColumnMapping,
    state: State<AppState>,
) -> Result<VibrationDataset, AppError> {
    let df = csv_reader::read_csv_with_mapping(&file_path, &column_mapping)?;

    let time_col = df.column("time")?.f64()?;
    let total_points = df.height();
    let time_min = time_col.min().unwrap_or(0.0);
    let time_max = time_col.max().unwrap_or(0.0);

    let id = Uuid::new_v4().to_string();
    let file_name = Path::new(&file_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| file_path.clone());

    let metadata = VibrationDataset {
        id: id.clone(),
        file_path: file_path.clone(),
        file_name,
        total_points,
        time_range: (time_min, time_max),
        column_mapping,
    };

    let mut datasets = state.datasets.write().map_err(|_| AppError::LockPoisoned)?;
    datasets.insert(
        id,
        DatasetEntry {
            metadata: metadata.clone(),
            dataframe: df,
        },
    );

    Ok(metadata)
}

#[tauri::command]
pub fn get_timeseries_chunk(
    dataset_id: String,
    start_time: f64,
    end_time: f64,
    max_points: usize,
    state: State<AppState>,
) -> Result<TimeseriesChunk, AppError> {
    let datasets = state.datasets.read().map_err(|_| AppError::LockPoisoned)?;
    let entry = datasets
        .get(&dataset_id)
        .ok_or_else(|| AppError::DatasetNotFound(dataset_id.clone()))?;
    let df = &entry.dataframe;
    let data_columns = &entry.metadata.column_mapping.data_columns;

    // Filter by time range using Polars lazy filter (SIMD-accelerated)
    let filtered = df
        .clone()
        .lazy()
        .filter(
            col("time")
                .gt_eq(lit(start_time))
                .and(col("time").lt_eq(lit(end_time))),
        )
        .collect()?;
    let original_count = filtered.height();

    let time_raw = extract_f64_vec(&filtered, "time")?;

    if original_count > max_points {
        let representative = if !data_columns.is_empty() {
            extract_f64_vec(&filtered, &data_columns[0])?
        } else {
            vec![0.0; time_raw.len()]
        };
        let indices = downsampling::lttb_indices(&time_raw, &representative, max_points);

        let time: Vec<f64> = indices.iter().map(|&i| time_raw[i]).collect();
        let mut channels = IndexMap::new();
        for col_name in data_columns {
            let raw = extract_f64_vec(&filtered, col_name)?;
            let sampled: Vec<f64> = indices.iter().map(|&i| raw[i]).collect();
            channels.insert(col_name.clone(), sampled);
        }

        Ok(TimeseriesChunk {
            time,
            channels,
            is_downsampled: true,
            original_count,
        })
    } else {
        let mut channels = IndexMap::new();
        for col_name in data_columns {
            channels.insert(col_name.clone(), extract_f64_vec(&filtered, col_name)?);
        }

        Ok(TimeseriesChunk {
            time: time_raw,
            channels,
            is_downsampled: false,
            original_count,
        })
    }
}

fn extract_f64_vec(df: &DataFrame, col_name: &str) -> Result<Vec<f64>, AppError> {
    Ok(df
        .column(col_name)?
        .f64()?
        .into_iter()
        .map(|opt| opt.unwrap_or(f64::NAN))
        .collect())
}
