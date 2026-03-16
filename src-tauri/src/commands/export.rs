use polars::prelude::*;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
pub fn export_data(
    dataset_id: String,
    output_path: String,
    start_time: Option<f64>,
    end_time: Option<f64>,
    state: State<AppState>,
) -> Result<String, String> {
    let datasets = state.datasets.lock().unwrap();
    let entry = datasets.get(&dataset_id).ok_or("Dataset not found")?;
    let df = &entry.dataframe;

    let export_df = match (start_time, end_time) {
        (Some(start), Some(end)) => {
            let mask = df
                .column("time")
                .unwrap()
                .f64()
                .unwrap()
                .into_iter()
                .map(|opt| opt.is_some_and(|t| t >= start && t <= end))
                .collect::<BooleanChunked>();
            df.filter(&mask).map_err(|e| e.to_string())?
        }
        _ => df.clone(),
    };

    let mut file =
        std::fs::File::create(&output_path).map_err(|e| format!("Cannot create file: {}", e))?;

    CsvWriter::new(&mut file)
        .finish(&mut export_df.clone())
        .map_err(|e| format!("CSV write failed: {}", e))?;

    Ok(output_path)
}
