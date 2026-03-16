export type AnnotationType =
	| { type: 'Point'; time: number; value: number; axis: string }
	| { type: 'Range'; start_time: number; end_time: number };

export interface Annotation {
	id: string;
	annotation_type: AnnotationType;
	label: string;
	color: string;
	label_offset_x: number;
	label_offset_y: number;
	created_at: string;
}
