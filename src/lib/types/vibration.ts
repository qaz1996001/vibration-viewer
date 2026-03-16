export interface VibrationDataset {
	id: string;
	file_path: string;
	total_points: number;
	time_range: [number, number];
	columns: string[];
}

export interface TimeseriesChunk {
	time: number[];
	x: number[];
	y: number[];
	z: number[];
	amplitude: number[];
	is_downsampled: boolean;
	original_count: number;
}
