use indexmap::IndexMap;
use polars::prelude::*;
use std::path::Path;
use tauri::State;
use uuid::Uuid;

use crate::error::AppError;
use crate::models::vibration::*;
use crate::services::{csv_reader, downsampling, time_filter};
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
    // Perform file I/O outside of lock scope
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

    // Acquire write lock only for the insert
    let mut datasets = state.datasets.write().map_err(|_| AppError::LockPoisoned)?;
    datasets.insert(
        id,
        DatasetEntry {
            metadata: metadata.clone(),
            dataframe: df,
        },
    );
    drop(datasets);

    // Update project context
    {
        let mut project_ctx = state.project.write().map_err(|_| AppError::LockPoisoned)?;
        if project_ctx.metadata.created_at.is_empty() {
            project_ctx.metadata.name = metadata.file_name.clone();
            project_ctx.metadata.created_at = "auto".into();
        }
        // Single-file mode with multi-file overlay; future: upgrade to AidpsFolder/VibprojFile
        project_ctx.project_type = crate::models::project::ProjectType::SingleFile;
    }

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
    // Acquire read lock, clone out needed data, then release lock before computation
    let (df_clone, data_columns) = {
        let datasets = state.datasets.read().map_err(|_| AppError::LockPoisoned)?;
        let entry = datasets
            .get(&dataset_id)
            .ok_or_else(|| AppError::DatasetNotFound(dataset_id.clone()))?;
        (entry.dataframe.clone(), entry.metadata.column_mapping.data_columns.clone())
    }; // Read lock released here

    // Filter by time range using shared time_filter (SIMD-accelerated)
    let filtered = time_filter::filter_time_range(&df_clone, "time", start_time, end_time)?;
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
        for col_name in &data_columns {
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
        for col_name in &data_columns {
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

/// Load all CSV files for a device (AIDPS multi-file merge).
/// Concatenates multiple CSVs into a single continuous timeseries,
/// sorted by time with duplicate timestamps removed.
///
/// Note: Uses `device_id` as the dataset key. Calling with the same
/// `device_id` will replace the previous entry (idempotent reload).
#[tauri::command]
pub fn load_device_data(
    device_id: String,
    file_paths: Vec<String>,
    column_mapping: ColumnMapping,
    state: State<AppState>,
) -> Result<VibrationDataset, AppError> {
    // Perform file I/O outside of lock scope
    let (df, time_min, time_max) = csv_reader::concat_csvs(&file_paths, &column_mapping)?;

    let total_points = df.height();

    let file_name = format!("{} ({} files)", device_id, file_paths.len());

    let metadata = VibrationDataset {
        id: device_id.clone(),
        // TODO: file_path stores only first file; multi-file provenance not yet tracked
        file_path: file_paths.first().cloned().unwrap_or_default(),
        file_name,
        total_points,
        time_range: (time_min, time_max),
        column_mapping,
    };

    // Acquire write lock only for the insert
    let mut datasets = state.datasets.write().map_err(|_| AppError::LockPoisoned)?;
    datasets.insert(
        device_id,
        DatasetEntry {
            metadata: metadata.clone(),
            dataframe: df,
        },
    );
    drop(datasets);

    // Update project context for AIDPS multi-file mode
    {
        let mut project_ctx = state.project.write().map_err(|_| AppError::LockPoisoned)?;
        if project_ctx.metadata.created_at.is_empty() {
            project_ctx.metadata.name = metadata.file_name.clone();
            project_ctx.metadata.created_at = "auto".into();
        }
        project_ctx.project_type = crate::models::project::ProjectType::AidpsFolder;
    }

    Ok(metadata)
}

fn extract_f64_vec(df: &DataFrame, col_name: &str) -> Result<Vec<f64>, AppError> {
    Ok(df
        .column(col_name)?
        .f64()?
        .into_iter()
        .map(|opt| opt.unwrap_or(f64::NAN))
        .collect())
}
