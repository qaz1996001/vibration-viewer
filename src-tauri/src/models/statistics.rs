use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsReport {
    pub basic: Vec<AxisBasicStats>,
    pub distribution: Vec<AxisDistributionStats>,
    pub shape: Vec<AxisShapeStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisBasicStats {
    pub axis: String,
    pub count: usize,
    pub mean: f64,
    pub std_dev: f64,
    pub cv_percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisDistributionStats {
    pub axis: String,
    pub min: f64,
    pub q1: f64,
    pub median: f64,
    pub q3: f64,
    pub max: f64,
    pub iqr: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisShapeStats {
    pub axis: String,
    pub skewness: f64,
    pub kurtosis: f64,
}
