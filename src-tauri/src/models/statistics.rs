//! 统计分析结果模型。
//!
//! 将每个 channel（axis）的统计指标分为三类：
//! - **Basic**：基础统计（均值、标准差、变异系数）
//! - **Distribution**：分布统计（五数概括 + IQR）
//! - **Shape**：形状统计（偏度、峰度）

use serde::{Deserialize, Serialize};

/// 完整统计报告，按 channel 聚合三类统计指标。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatisticsReport {
    /// 各 channel 的基础统计
    pub basic: Vec<AxisBasicStats>,
    /// 各 channel 的分布统计
    pub distribution: Vec<AxisDistributionStats>,
    /// 各 channel 的形状统计
    pub shape: Vec<AxisShapeStats>,
}

/// 单个 channel 的基础统计指标。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisBasicStats {
    /// Channel 名称
    pub axis: String,
    /// 数据点数
    pub count: usize,
    /// 算术平均值
    pub mean: f64,
    /// 标准差（sample std dev）
    pub std_dev: f64,
    /// 变异系数百分比（CV% = std_dev / mean * 100）
    pub cv_percent: f64,
}

/// 单个 channel 的分布统计指标（五数概括）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisDistributionStats {
    /// Channel 名称
    pub axis: String,
    /// 最小值
    pub min: f64,
    /// 第一四分位数（Q1, 25th percentile）
    pub q1: f64,
    /// 中位数（Q2, 50th percentile）
    pub median: f64,
    /// 第三四分位数（Q3, 75th percentile）
    pub q3: f64,
    /// 最大值
    pub max: f64,
    /// 四分位距（IQR = Q3 - Q1）
    pub iqr: f64,
}

/// 单个 channel 的形状统计指标。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AxisShapeStats {
    /// Channel 名称
    pub axis: String,
    /// 偏度（skewness），衡量分布的不对称程度
    pub skewness: f64,
    /// 峰度（kurtosis），衡量分布的尖锐程度
    pub kurtosis: f64,
}
