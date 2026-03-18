/**
 * 振動資料核心型別定義
 *
 * 定義 CSV 欄位映射、預覽、資料集及時序片段等 IPC 傳輸的資料結構。
 * 所有 interface 與 Rust 後端 models/vibration.rs 的 serde 序列化結果一一對應。
 */

/** CSV 欄位映射 — 使用者在 ColumnMappingDialog 中選擇時間欄和資料欄後產生 */
export interface ColumnMapping {
	/** 時間軸欄位名稱（epoch seconds 或可解析的 datetime 字串） */
	time_column: string;
	/** 使用者選取的數值資料欄位名稱列表（動態，非固定 x/y/z） */
	data_columns: string[];
}

/** CSV 檔案預覽 — preview_csv_columns 回傳，用於 ColumnMappingDialog 顯示 */
export interface CsvPreview {
	/** CSV 檔案的完整路徑 */
	file_path: string;
	/** CSV 所有欄位名稱（保持檔案中的原始順序） */
	columns: string[];
	/** 資料列總數（不含 header） */
	row_count: number;
}

/** 已載入的振動資料集元資料 — load_vibration_data 回傳 */
export interface VibrationDataset {
	/** 後端產生的唯一識別碼（UUID） */
	id: string;
	/** CSV 檔案的完整路徑 */
	file_path: string;
	/** 顯示用檔名（不含路徑） */
	file_name: string;
	/** 資料集內的總資料點數 */
	total_points: number;
	/** 時間範圍 [起始秒, 結束秒]（epoch seconds） */
	time_range: [number, number];
	/** 載入時使用的欄位映射設定 */
	column_mapping: ColumnMapping;
}

/** 時序資料片段 — get_timeseries_chunk 回傳，供 ECharts 繪製 */
export interface TimeseriesChunk {
	/** 時間戳陣列（epoch seconds），與 channels 中每個陣列等長且索引對齊 */
	time: number[];
	/** 各通道的數值陣列，key 為通道名稱（保持 IndexMap 順序） */
	channels: Record<string, number[]>;
	/** 是否經 LTTB 降採樣（true 表示非完整資料） */
	is_downsampled: boolean;
	/** 降採樣前的原始資料點數（用於 UI 顯示 "N / M points"） */
	original_count: number;
}
