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
