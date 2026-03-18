/**
 * 統計報表型別定義
 *
 * 對應 Rust 後端 models/statistics.rs 的 compute_statistics 回傳結構。
 * 每個通道（axis/channel）獨立計算基礎統計量、分佈統計量及形狀統計量。
 */

/** 完整統計報表 — 包含所有通道的三類統計 */
export interface StatisticsReport {
	/** 基礎統計量（均值、標準差等） */
	basic: AxisBasicStats[];
	/** 分佈統計量（四分位數、極值等） */
	distribution: AxisDistributionStats[];
	/** 形狀統計量（偏度、峰度） */
	shape: AxisShapeStats[];
}

/** 單一通道的基礎統計量 */
export interface AxisBasicStats {
	/** 通道名稱（對應 ColumnMapping.data_columns 中的欄位） */
	axis: string;
	/** 資料點數 */
	count: number;
	/** 算術平均值 */
	mean: number;
	/** 標準差（母體標準差） */
	std_dev: number;
	/** 變異係數（百分比），CV = std_dev / mean * 100 */
	cv_percent: number;
}

/** 單一通道的分佈統計量（五數摘要 + IQR） */
export interface AxisDistributionStats {
	/** 通道名稱 */
	axis: string;
	/** 最小值 */
	min: number;
	/** 第一四分位數（25th percentile） */
	q1: number;
	/** 中位數（50th percentile） */
	median: number;
	/** 第三四分位數（75th percentile） */
	q3: number;
	/** 最大值 */
	max: number;
	/** 四分位距，IQR = Q3 - Q1 */
	iqr: number;
}

/** 單一通道的形狀統計量 */
export interface AxisShapeStats {
	/** 通道名稱 */
	axis: string;
	/** 偏度（skewness）— 正值右偏、負值左偏 */
	skewness: number;
	/** 峰度（kurtosis）— 衡量分佈尾部厚度，常態分佈為 3 */
	kurtosis: number;
}
