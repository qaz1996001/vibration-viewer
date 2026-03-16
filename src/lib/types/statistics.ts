export interface StatisticsReport {
	basic: AxisBasicStats[];
	distribution: AxisDistributionStats[];
	shape: AxisShapeStats[];
}

export interface AxisBasicStats {
	axis: string;
	count: number;
	mean: number;
	std_dev: number;
	cv_percent: number;
}

export interface AxisDistributionStats {
	axis: string;
	min: number;
	q1: number;
	median: number;
	q3: number;
	max: number;
	iqr: number;
}

export interface AxisShapeStats {
	axis: string;
	skewness: number;
	kurtosis: number;
}
