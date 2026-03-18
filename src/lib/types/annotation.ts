/**
 * 標註型別定義
 *
 * 對應 Rust 後端 models/annotation.rs 的序列化結構。
 * 以 discriminated union 區分 Point（單點標註）與 Range（區間標註）。
 * 標註以 JSON 格式儲存於 {datafile}.vibann.json。
 */

/**
 * 標註類型 — discriminated union
 *
 * - `Point`: 單一時間點標註，帶有所屬通道（axis）和數值
 * - `Range`: 時間區間標註，標記起訖時間
 */
export type AnnotationType =
	| { type: 'Point'; time: number; value: number; axis: string }
	| { type: 'Range'; start_time: number; end_time: number };

/** 單一標註項目，包含位置、顯示屬性及 label 位移 */
export interface Annotation {
	/** 唯一識別碼（UUID，前端產生） */
	id: string;
	/** 標註的位置資訊（Point 或 Range） */
	annotation_type: AnnotationType;
	/** 使用者輸入的標籤文字 */
	label: string;
	/** 標註顏色（CSS hex 色碼） */
	color: string;
	/** Label 相對於標註位置的水平偏移量（px，可拖曳調整） */
	label_offset_x: number;
	/** Label 相對於標註位置的垂直偏移量（px，可拖曳調整） */
	label_offset_y: number;
	/** 建立時間（ISO 8601 字串） */
	created_at: string;
}
