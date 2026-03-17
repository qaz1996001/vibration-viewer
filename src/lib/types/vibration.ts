export interface ColumnMapping {
	time_column: string;
	data_columns: string[];
}

export interface CsvPreview {
	file_path: string;
	columns: string[];
	row_count: number;
}

export interface VibrationDataset {
	id: string;
	file_path: string;
	file_name: string;
	total_points: number;
	time_range: [number, number];
	column_mapping: ColumnMapping;
}

export interface TimeseriesChunk {
	time: number[];
	channels: Record<string, number[]>;
	is_downsampled: boolean;
	original_count: number;
}
