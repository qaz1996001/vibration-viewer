use polars::prelude::*;

use crate::error::AppError;
use crate::models::statistics::*;

pub fn compute_basic_stats(series: &Series, axis_name: &str) -> Result<AxisBasicStats, AppError> {
    let count = series.len();

    // Edge case: empty series
    if count == 0 {
        return Ok(AxisBasicStats {
            axis: axis_name.to_string(),
            count: 0,
            mean: f64::NAN,
            std_dev: f64::NAN,
            cv_percent: f64::NAN,
        });
    }

    let mean = series.mean().unwrap_or(f64::NAN);
    let std_dev = series.std(1).unwrap_or(f64::NAN);
    let cv_percent = if mean.abs() > f64::EPSILON && !mean.is_nan() && !std_dev.is_nan() {
        (std_dev / mean.abs()) * 100.0
    } else {
        f64::NAN
    };

    Ok(AxisBasicStats {
        axis: axis_name.to_string(),
        count,
        mean,
        std_dev,
        cv_percent,
    })
}

pub fn compute_distribution_stats(
    series: &Series,
    axis_name: &str,
) -> Result<AxisDistributionStats, AppError> {
    let n = series.len();

    // Edge case: empty series
    if n == 0 {
        return Ok(AxisDistributionStats {
            axis: axis_name.to_string(),
            min: f64::NAN,
            q1: f64::NAN,
            median: f64::NAN,
            q3: f64::NAN,
            max: f64::NAN,
            iqr: f64::NAN,
        });
    }

    let sorted = series
        .sort(SortOptions::default())
        .map_err(|e| AppError::Statistics(format!("sort failed: {e}")))?;
    let min = sorted.min::<f64>().unwrap_or(Some(f64::NAN)).unwrap_or(f64::NAN);
    let max = sorted.max::<f64>().unwrap_or(Some(f64::NAN)).unwrap_or(f64::NAN);
    let median = sorted.median().unwrap_or(f64::NAN);

    let q1 = percentile(&sorted, 25.0)?;
    let q3 = percentile(&sorted, 75.0)?;

    Ok(AxisDistributionStats {
        axis: axis_name.to_string(),
        min,
        q1,
        median,
        q3,
        max,
        iqr: q3 - q1,
    })
}

/// NOTE: Polars 0.46 provides built-in `Series::skew(bias)` and
/// `Series::kurtosis(fisher, bias)` via the `moment` feature flag
/// (in polars-ops, trait `MomentSeries`). These use scipy-equivalent
/// moment-based formulas and support bias correction. Consider switching
/// to the built-in methods when this module is next refactored.
pub fn compute_shape_stats(series: &Series, axis_name: &str) -> Result<AxisShapeStats, AppError> {
    let n = series.len() as f64;

    // Edge case: empty series or fewer than 3 points
    if n < 1.0 {
        return Ok(AxisShapeStats {
            axis: axis_name.to_string(),
            skewness: f64::NAN,
            kurtosis: f64::NAN,
        });
    }

    let mean = series.mean().unwrap_or(f64::NAN);
    let std_dev = series.std(1).unwrap_or(f64::NAN);

    // Guard: if std_dev is zero, NaN, or too few points, skewness/kurtosis are undefined
    if std_dev == 0.0 || std_dev.is_nan() || n < 3.0 {
        return Ok(AxisShapeStats {
            axis: axis_name.to_string(),
            skewness: f64::NAN,
            kurtosis: f64::NAN,
        });
    }

    let ca = series
        .f64()
        .map_err(|e| AppError::Statistics(format!("expected f64 series: {e}")))?;
    let mut sum3 = 0.0;
    let mut sum4 = 0.0;

    for val in ca.into_iter().flatten() {
        let z = (val - mean) / std_dev;
        sum3 += z.powi(3);
        sum4 += z.powi(4);
    }

    let skewness = sum3 / n;
    let kurtosis = (sum4 / n) - 3.0;

    Ok(AxisShapeStats {
        axis: axis_name.to_string(),
        skewness,
        kurtosis,
    })
}

fn percentile(sorted_series: &Series, pct: f64) -> Result<f64, AppError> {
    let ca = sorted_series
        .f64()
        .map_err(|e| AppError::Statistics(format!("expected f64 series: {e}")))?;
    let n = ca.len();
    if n == 0 {
        return Ok(f64::NAN);
    }
    let rank = (pct / 100.0) * (n - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    let frac = rank - lower as f64;

    let lower_val = ca.get(lower).unwrap_or(f64::NAN);
    let upper_val = ca.get(upper).unwrap_or(f64::NAN);

    Ok(lower_val + frac * (upper_val - lower_val))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a Float64 Series from a slice.
    fn f64_series(name: &str, values: &[f64]) -> Series {
        Series::new(name.into(), values)
    }

    /// Helper: create a Float64 Series with explicit nulls.
    fn f64_series_with_nulls(name: &str, values: &[Option<f64>]) -> Series {
        let ca = Float64Chunked::new(name.into(), values);
        ca.into_series()
    }

    // ─── compute_basic_stats ───

    #[test]
    fn test_basic_stats_normal_data() {
        // [1, 2, 3, 4, 5] => mean=3.0, std_dev=sqrt(2.5)=1.5811...
        let s = f64_series("x", &[1.0, 2.0, 3.0, 4.0, 5.0]);
        let stats = compute_basic_stats(&s, "x").unwrap();
        assert_eq!(stats.axis, "x");
        assert_eq!(stats.count, 5);
        assert!((stats.mean - 3.0).abs() < 1e-10);
        // sample std dev (ddof=1): sqrt(10/4) = sqrt(2.5) ≈ 1.5811
        assert!((stats.std_dev - 2.5_f64.sqrt()).abs() < 1e-10);
        // cv = std_dev / |mean| * 100
        let expected_cv = (2.5_f64.sqrt() / 3.0) * 100.0;
        assert!((stats.cv_percent - expected_cv).abs() < 1e-6);
    }

    #[test]
    fn test_basic_stats_single_value() {
        let s = f64_series("x", &[42.0]);
        let stats = compute_basic_stats(&s, "x").unwrap();
        assert_eq!(stats.count, 1);
        assert!((stats.mean - 42.0).abs() < 1e-10);
        // std_dev of single value with ddof=1 is 0 (Polars returns 0 for len=1)
        assert!(stats.std_dev.abs() < 1e-10 || stats.std_dev.is_nan());
    }

    #[test]
    fn test_basic_stats_constant_series() {
        let s = f64_series("x", &[5.0, 5.0, 5.0, 5.0]);
        let stats = compute_basic_stats(&s, "x").unwrap();
        assert!((stats.mean - 5.0).abs() < 1e-10);
        assert!(stats.std_dev.abs() < 1e-10);
        // cv should be 0 since std_dev is 0
        assert!(stats.cv_percent.abs() < 1e-10);
    }

    #[test]
    fn test_basic_stats_negative_values() {
        let s = f64_series("y", &[-10.0, -20.0, -30.0]);
        let stats = compute_basic_stats(&s, "y").unwrap();
        assert!((stats.mean - (-20.0)).abs() < 1e-10);
        assert!(stats.std_dev > 0.0);
    }

    #[test]
    fn test_basic_stats_very_large_values() {
        let s = f64_series("x", &[1e15, 2e15, 3e15]);
        let stats = compute_basic_stats(&s, "x").unwrap();
        assert!((stats.mean - 2e15).abs() < 1e5);
        assert!(stats.std_dev.is_finite());
        assert!(stats.cv_percent.is_finite());
    }

    #[test]
    fn test_basic_stats_mean_near_zero_cv_fallback() {
        // When mean is near zero, cv_percent should be NaN (not Inf)
        let s = f64_series("x", &[-1.0, 1.0]);
        let stats = compute_basic_stats(&s, "x").unwrap();
        assert!(stats.mean.abs() < 1e-10);
        assert!(stats.cv_percent.is_nan(), "cv_percent should be NaN when mean is near zero");
    }

    // ─── compute_distribution_stats ───

    #[test]
    fn test_distribution_stats_known_values() {
        // [1, 2, 3, 4, 5] sorted: min=1, max=5, median=3
        let s = f64_series("x", &[5.0, 1.0, 3.0, 2.0, 4.0]);
        let stats = compute_distribution_stats(&s, "x").unwrap();
        assert!((stats.min - 1.0).abs() < 1e-10);
        assert!((stats.max - 5.0).abs() < 1e-10);
        assert!((stats.median - 3.0).abs() < 1e-10);
        assert!(stats.iqr > 0.0);
        assert!((stats.iqr - (stats.q3 - stats.q1)).abs() < 1e-10);
    }

    #[test]
    fn test_distribution_stats_single_value() {
        let s = f64_series("x", &[7.0]);
        let stats = compute_distribution_stats(&s, "x").unwrap();
        assert!((stats.min - 7.0).abs() < 1e-10);
        assert!((stats.max - 7.0).abs() < 1e-10);
        assert!((stats.median - 7.0).abs() < 1e-10);
        assert!(stats.iqr.abs() < 1e-10);
    }

    #[test]
    fn test_distribution_stats_two_values() {
        let s = f64_series("x", &[10.0, 20.0]);
        let stats = compute_distribution_stats(&s, "x").unwrap();
        assert!((stats.min - 10.0).abs() < 1e-10);
        assert!((stats.max - 20.0).abs() < 1e-10);
    }

    // ─── compute_shape_stats ───

    #[test]
    fn test_shape_stats_symmetric_data() {
        // Symmetric distribution: skewness should be near 0
        let s = f64_series("x", &[-2.0, -1.0, 0.0, 1.0, 2.0]);
        let stats = compute_shape_stats(&s, "x").unwrap();
        assert!(stats.skewness.abs() < 1e-10);
    }

    #[test]
    fn test_shape_stats_constant_series_returns_nan() {
        // std_dev==0 means skewness/kurtosis are undefined
        let s = f64_series("x", &[3.0, 3.0, 3.0, 3.0]);
        let stats = compute_shape_stats(&s, "x").unwrap();
        assert!(stats.skewness.is_nan(), "skewness should be NaN when std_dev==0");
        assert!(stats.kurtosis.is_nan(), "kurtosis should be NaN when std_dev==0");
    }

    #[test]
    fn test_shape_stats_single_value_returns_nan() {
        // n<3 means skewness/kurtosis are undefined
        let s = f64_series("x", &[42.0]);
        let stats = compute_shape_stats(&s, "x").unwrap();
        assert!(stats.skewness.is_nan(), "skewness should be NaN for n<3");
        assert!(stats.kurtosis.is_nan(), "kurtosis should be NaN for n<3");
    }

    #[test]
    fn test_shape_stats_two_values_returns_nan() {
        // n < 3 guard triggers
        let s = f64_series("x", &[1.0, 2.0]);
        let stats = compute_shape_stats(&s, "x").unwrap();
        assert!(stats.skewness.is_nan(), "skewness should be NaN for n<3");
        assert!(stats.kurtosis.is_nan(), "kurtosis should be NaN for n<3");
    }

    #[test]
    fn test_shape_stats_skewed_data() {
        // Right-skewed data: mostly small values with one large outlier
        let s = f64_series("x", &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 100.0]);
        let stats = compute_shape_stats(&s, "x").unwrap();
        assert!(stats.skewness > 0.0, "Expected positive skewness for right-skewed data");
    }

    #[test]
    fn test_shape_stats_nan_values_skipped() {
        // NaN values should be skipped via .flatten()
        let s = f64_series_with_nulls("x", &[Some(1.0), None, Some(2.0), None, Some(3.0)]);
        let stats = compute_shape_stats(&s, "x").unwrap();
        // Should not panic; values should be finite
        assert!(stats.skewness.is_finite());
        assert!(stats.kurtosis.is_finite());
    }

    // ─── percentile helper ───

    #[test]
    fn test_percentile_quartiles() {
        // [0, 25, 50, 75, 100] => Q1=25, median=50, Q3=75
        let s = f64_series("x", &[0.0, 25.0, 50.0, 75.0, 100.0]);
        let sorted = s.sort(SortOptions::default()).unwrap();
        assert!((percentile(&sorted, 0.0).unwrap() - 0.0).abs() < 1e-10);
        assert!((percentile(&sorted, 25.0).unwrap() - 25.0).abs() < 1e-10);
        assert!((percentile(&sorted, 50.0).unwrap() - 50.0).abs() < 1e-10);
        assert!((percentile(&sorted, 75.0).unwrap() - 75.0).abs() < 1e-10);
        assert!((percentile(&sorted, 100.0).unwrap() - 100.0).abs() < 1e-10);
    }

    #[test]
    fn test_percentile_empty_series() {
        let s = f64_series("x", &[]);
        assert!(percentile(&s, 50.0).unwrap().is_nan(), "percentile of empty series should be NaN");
    }
}
