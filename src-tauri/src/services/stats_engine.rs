use polars::prelude::*;

use crate::models::statistics::*;

pub fn compute_basic_stats(series: &Series, axis_name: &str) -> AxisBasicStats {
    let count = series.len();
    let mean = series.mean().unwrap_or(0.0);
    let std_dev = series.std(1).unwrap_or(0.0);
    let cv_percent = if mean.abs() > f64::EPSILON {
        (std_dev / mean.abs()) * 100.0
    } else {
        0.0
    };

    AxisBasicStats {
        axis: axis_name.to_string(),
        count,
        mean,
        std_dev,
        cv_percent,
    }
}

pub fn compute_distribution_stats(series: &Series, axis_name: &str) -> AxisDistributionStats {
    let sorted = series
        .sort(SortOptions::default())
        .unwrap_or_else(|_| series.clone());
    let min = sorted.min::<f64>().unwrap_or(Some(0.0)).unwrap_or(0.0);
    let max = sorted.max::<f64>().unwrap_or(Some(0.0)).unwrap_or(0.0);
    let median = sorted.median().unwrap_or(0.0);

    let q1 = percentile(&sorted, 25.0);
    let q3 = percentile(&sorted, 75.0);

    AxisDistributionStats {
        axis: axis_name.to_string(),
        min,
        q1,
        median,
        q3,
        max,
        iqr: q3 - q1,
    }
}

pub fn compute_shape_stats(series: &Series, axis_name: &str) -> AxisShapeStats {
    let mean = series.mean().unwrap_or(0.0);
    let std_dev = series.std(1).unwrap_or(0.0);
    let n = series.len() as f64;

    // Guard: constant series has zero skewness and kurtosis
    if std_dev < f64::EPSILON || n < 3.0 {
        return AxisShapeStats {
            axis: axis_name.to_string(),
            skewness: 0.0,
            kurtosis: 0.0,
        };
    }

    // Safety: caller validates column is f64 before calling
    let ca = series.f64().unwrap();
    let mut sum3 = 0.0;
    let mut sum4 = 0.0;

    for val in ca.into_iter().flatten() {
        let z = (val - mean) / std_dev;
        sum3 += z.powi(3);
        sum4 += z.powi(4);
    }

    let skewness = sum3 / n;
    let kurtosis = (sum4 / n) - 3.0;

    AxisShapeStats {
        axis: axis_name.to_string(),
        skewness,
        kurtosis,
    }
}

fn percentile(sorted_series: &Series, pct: f64) -> f64 {
    // Safety: caller validates column is f64 before calling
    let ca = sorted_series.f64().unwrap();
    let n = ca.len();
    if n == 0 {
        return 0.0;
    }
    let rank = (pct / 100.0) * (n - 1) as f64;
    let lower = rank.floor() as usize;
    let upper = rank.ceil() as usize;
    let frac = rank - lower as f64;

    let lower_val = ca.get(lower).unwrap_or(0.0);
    let upper_val = ca.get(upper).unwrap_or(0.0);

    lower_val + frac * (upper_val - lower_val)
}
