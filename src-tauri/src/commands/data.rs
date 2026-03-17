use std::collections::HashMap;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

use crate::models::vibration::*;
use crate::services::{csv_reader, downsampling};
use crate::state::{AppState, DatasetEntry};

#[tauri::command]
pub fn preview_csv_columns(file_path: String) -> Result<CsvPreview, String> {
    csv_reader::preview_csv(&file_path)
}

#[tauri::command]
pub fn load_vibration_data(
    file_path: String,
    column_mapping: ColumnMapping,
    state: State<AppState>,
) -> Result<VibrationDataset, String> {
    let df = csv_reader::read_csv_with_mapping(&file_path, &column_mapping)?;

    let time_col = df.column("time").map_err(|e| e.to_string())?;
    let time_ca = time_col.f64().map_err(|e| e.to_string())?;
    let total_points = df.height();
    let time_vec: Vec<f64> = time_ca.into_no_null_iter().collect();
    let time_min = time_vec.iter().cloned().reduce(f64::min).unwrap_or(0.0);
    let time_max = time_vec.iter().cloned().reduce(f64::max).unwrap_or(0.0);

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

    let mut datasets = state.datasets.lock().unwrap();
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
) -> Result<TimeseriesChunk, String> {
    let datasets = state.datasets.lock().unwrap();
    let entry = datasets.get(&dataset_id).ok_or("Dataset not found")?;
    let df = &entry.dataframe;
    let data_columns = &entry.metadata.column_mapping.data_columns;

    // Filter by time range
    let mask = df
        .column("time")
        .unwrap()
        .f64()
        .unwrap()
        .into_iter()
        .map(|opt| opt.is_some_and(|t| t >= start_time && t <= end_time))
        .collect::<polars::prelude::BooleanChunked>();

    let filtered = df.filter(&mask).map_err(|e| e.to_string())?;
    let original_count = filtered.height();

    let time_raw = extract_f64_vec(&filtered, "time");

    if original_count > max_points {
        // Use first data column as representative for LTTB index selection
        let representative = if !data_columns.is_empty() {
            extract_f64_vec(&filtered, &data_columns[0])
        } else {
            vec![0.0; time_raw.len()]
        };
        let indices = downsampling::lttb_indices(&time_raw, &representative, max_points);

        let time: Vec<f64> = indices.iter().map(|&i| time_raw[i]).collect();
        let mut channels = HashMap::new();
        for col_name in data_columns {
            let raw = extract_f64_vec(&filtered, col_name);
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
        let mut channels = HashMap::new();
        for col_name in data_columns {
            channels.insert(col_name.clone(), extract_f64_vec(&filtered, col_name));
        }

        Ok(TimeseriesChunk {
            time: time_raw,
            channels,
            is_downsampled: false,
            original_count,
        })
    }
}

fn extract_f64_vec(df: &polars::prelude::DataFrame, col_name: &str) -> Vec<f64> {
    df.column(col_name)
        .unwrap()
        .f64()
        .unwrap()
        .into_no_null_iter()
        .collect()
}
