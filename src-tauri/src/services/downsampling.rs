/// LTTB (Largest-Triangle-Three-Buckets) downsampling.
/// Reduces N points to `threshold` points while preserving visual features.
#[cfg(test)]
pub fn lttb(time: &[f64], values: &[f64], threshold: usize) -> (Vec<f64>, Vec<f64>) {
    let indices = lttb_indices(time, values, threshold);
    let sampled_time: Vec<f64> = indices.iter().map(|&i| time[i]).collect();
    let sampled_values: Vec<f64> = indices.iter().map(|&i| values[i]).collect();
    (sampled_time, sampled_values)
}

/// NOTE: This function uses a single representative channel for index selection.
/// The selected indices are then applied to ALL channels. This means a spike in
/// channel Z that doesn't appear in the representative channel may be missed in
/// the downsampled output. This is an intentional tradeoff for simplicity —
/// multi-channel LTTB (union of per-channel indices) can be added if users
/// report missing peaks in specific channels.
///
/// LTTB that returns selected indices instead of values.
/// Use one representative channel for index selection, then apply indices to all channels.
pub fn lttb_indices(time: &[f64], values: &[f64], threshold: usize) -> Vec<usize> {
    let n = time.len();

    if threshold >= n || threshold < 3 {
        return (0..n).collect();
    }

    let mut indices = Vec::with_capacity(threshold);

    // Always keep first point
    indices.push(0);

    let bucket_size = (n - 2) as f64 / (threshold - 2) as f64;
    let mut prev_index = 0usize;

    for i in 1..(threshold - 1) {
        let bucket_start = ((i - 1) as f64 * bucket_size).floor() as usize + 1;
        let bucket_end = (i as f64 * bucket_size).floor() as usize + 1;
        let bucket_end = bucket_end.min(n - 1);

        // Next bucket average (third vertex of triangle)
        let next_start = bucket_end;
        let next_end = ((i + 1) as f64 * bucket_size).floor() as usize + 1;
        let next_end = next_end.min(n);

        let count = (next_end - next_start) as f64;
        let avg_time: f64 = time[next_start..next_end].iter().sum::<f64>() / count;
        let avg_value: f64 = values[next_start..next_end].iter().sum::<f64>() / count;

        // Find point with max triangle area in current bucket
        let mut max_area = -1.0f64;
        let mut max_index = bucket_start;

        let prev_time = time[prev_index];
        let prev_value = values[prev_index];

        for j in bucket_start..bucket_end {
            let area = ((prev_time - avg_time) * (values[j] - prev_value)
                - (prev_time - time[j]) * (avg_value - prev_value))
                .abs()
                * 0.5;

            if area > max_area {
                max_area = area;
                max_index = j;
            }
        }

        indices.push(max_index);
        prev_index = max_index;
    }

    // Always keep last point
    indices.push(n - 1);

    indices
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lttb_preserves_length_when_below_threshold() {
        let time = vec![1.0, 2.0, 3.0];
        let values = vec![10.0, 20.0, 30.0];
        let (t, v) = lttb(&time, &values, 10);
        assert_eq!(t.len(), 3);
        assert_eq!(v.len(), 3);
    }

    #[test]
    fn test_lttb_reduces_to_threshold() {
        let time: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let values: Vec<f64> = time.iter().map(|t| (t * 0.1).sin()).collect();
        let (t, v) = lttb(&time, &values, 100);
        assert_eq!(t.len(), 100);
        assert_eq!(v.len(), 100);
        assert_eq!(t[0], 0.0);
        assert_eq!(t[99], 999.0);
    }

    #[test]
    fn test_lttb_preserves_first_and_last() {
        let time: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let values: Vec<f64> = time.iter().map(|t| t * t).collect();
        let (t, _) = lttb(&time, &values, 20);
        assert_eq!(t[0], time[0]);
        assert_eq!(*t.last().unwrap(), *time.last().unwrap());
    }

    #[test]
    fn test_lttb_indices_returns_correct_count() {
        let time: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let values: Vec<f64> = time.iter().map(|t| (t * 0.1).sin()).collect();
        let indices = lttb_indices(&time, &values, 100);
        assert_eq!(indices.len(), 100);
        assert_eq!(indices[0], 0);
        assert_eq!(*indices.last().unwrap(), 999);
    }

    #[test]
    fn test_lttb_indices_below_threshold() {
        let time = vec![1.0, 2.0, 3.0];
        let values = vec![10.0, 20.0, 30.0];
        let indices = lttb_indices(&time, &values, 10);
        assert_eq!(indices, vec![0, 1, 2]);
    }
}
