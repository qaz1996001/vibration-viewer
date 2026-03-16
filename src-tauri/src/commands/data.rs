use polars::prelude::*;
use tauri::State;
use uuid::Uuid;

use crate::models::vibration::*;
use crate::services::{csv_reader, downsampling};
use crate::state::{AppState, DatasetEntry};

#[tauri::command]
pub fn load_vibration_data(
    file_path: String,
    state: State<AppState>,
) -> Result<VibrationDataset, String> {
    let df = csv_reader::read_vibration_csv(&file_path)?;

    let time_col = df.column("time").map_err(|e| e.to_string())?;
    let time_ca = time_col.f64().map_err(|e| e.to_string())?;
    let total_points = df.height();
    let time_min = time_ca.min().unwrap_or(0.0);
    let time_max = time_ca.max().unwrap_or(0.0);

    let id = Uuid::new_v4().to_string();
    let columns: Vec<String> = df
        .get_column_names()
        .iter()
        .map(|s| s.to_string())
        .collect();

    let metadata = VibrationDataset {
        id: id.clone(),
        file_path: file_path.clone(),
        total_points,
        time_range: (time_min, time_max),
        columns,
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

    // Filter by time range
    let mask = df
        .column("time")
        .unwrap()
        .f64()
        .unwrap()
        .into_iter()
        .map(|opt| opt.is_some_and(|t| t >= start_time && t <= end_time))
        .collect::<BooleanChunked>();

    let filtered = df.filter(&mask).map_err(|e| e.to_string())?;
    let original_count = filtered.height();

    let time_raw = extract_f64_vec(&filtered, "time");
    let x_raw = extract_f64_vec(&filtered, "x");
    let y_raw = extract_f64_vec(&filtered, "y");
    let z_raw = extract_f64_vec(&filtered, "z");
    let amp_raw = extract_f64_vec(&filtered, "amplitude");

    if original_count > max_points {
        let (time, x) = downsampling::lttb(&time_raw, &x_raw, max_points);
        let (_, y) = downsampling::lttb(&time_raw, &y_raw, max_points);
        let (_, z) = downsampling::lttb(&time_raw, &z_raw, max_points);
        let (_, amplitude) = downsampling::lttb(&time_raw, &amp_raw, max_points);

        Ok(TimeseriesChunk {
            time,
            x,
            y,
            z,
            amplitude,
            is_downsampled: true,
            original_count,
        })
    } else {
        Ok(TimeseriesChunk {
            time: time_raw,
            x: x_raw,
            y: y_raw,
            z: z_raw,
            amplitude: amp_raw,
            is_downsampled: false,
            original_count,
        })
    }
}

fn extract_f64_vec(df: &DataFrame, col_name: &str) -> Vec<f64> {
    df.column(col_name)
        .unwrap()
        .f64()
        .unwrap()
        .into_no_null_iter()
        .collect()
}
