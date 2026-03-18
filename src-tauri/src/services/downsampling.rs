//! LTTB (Largest-Triangle-Three-Buckets) 降采样算法。
//!
//! 将 N 个时间序列数据点缩减为 `threshold` 个点，同时保留视觉上显著的特征
//! （极值、拐点等）。相比简单的等间距采样或平均值采样，LTTB 能更好地保持
//! 原始波形的视觉特征。
//!
//! 核心思想: 将数据分为 `threshold - 2` 个等宽 bucket，在每个 bucket 中选择
//! 使得三角形面积最大的点（三角形顶点为: 上一个选中点、候选点、下一个 bucket 均值）。
//!
//! 本模块提供基于 **索引** 的接口 ([`lttb_indices`])，返回选中点的原始索引，
//! 而非复制数据。这样可以用同一组索引对所有 channel 进行对齐降采样。
//!
//! 参考: Sveinn Steinarsson, "Downsampling Time Series for Visual Representation" (2013)

/// LTTB 降采样，返回采样后的 `(time, values)` 向量。
///
/// 仅在测试中使用，生产代码应使用 [`lttb_indices`] 获取索引后自行取值。
#[cfg(test)]
pub fn lttb(time: &[f64], values: &[f64], threshold: usize) -> (Vec<f64>, Vec<f64>) {
    let indices = lttb_indices(time, values, threshold);
    let sampled_time: Vec<f64> = indices.iter().map(|&i| time[i]).collect();
    let sampled_values: Vec<f64> = indices.iter().map(|&i| values[i]).collect();
    (sampled_time, sampled_values)
}

/// LTTB 降采样，返回选中点的原始索引而非数据值。
///
/// 使用单个代表 channel (`values`) 计算索引，然后将相同索引应用于所有 channel。
/// 这确保了多 channel 时间对齐，但某个 channel 独有的极值可能被遗漏。
///
/// # Parameters
/// - `time`: 时间戳数组（必须单调递增）
/// - `values`: 代表 channel 的数据值数组（与 `time` 等长）
/// - `threshold`: 目标点数。若 `threshold >= n` 或 `threshold < 3`，直接返回全部索引。
///
/// # Returns
/// 严格递增的索引向量，长度为 `min(threshold, n)`。始终包含首尾点。
///
/// # 已知限制
/// 仅基于单 channel 选择索引。若需多 channel 感知的降采样（取各 channel
/// 索引的并集），可在此基础上扩展。目前作为简单性与正确性的权衡。
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

    #[test]
    fn test_lttb_indices_exact_threshold() {
        // When threshold == n, should return all indices
        let time: Vec<f64> = (0..50).map(|i| i as f64).collect();
        let values: Vec<f64> = time.iter().map(|t| t * 2.0).collect();
        let indices = lttb_indices(&time, &values, 50);
        let expected: Vec<usize> = (0..50).collect();
        assert_eq!(indices, expected);
    }

    #[test]
    fn test_lttb_empty_input() {
        let time: Vec<f64> = vec![];
        let values: Vec<f64> = vec![];
        let indices = lttb_indices(&time, &values, 10);
        assert!(indices.is_empty());
    }

    #[test]
    fn test_lttb_single_point() {
        let time = vec![1.0];
        let values = vec![5.0];
        let indices = lttb_indices(&time, &values, 10);
        assert_eq!(indices, vec![0]);
    }

    #[test]
    fn test_lttb_two_points() {
        let time = vec![1.0, 2.0];
        let values = vec![5.0, 10.0];
        let indices = lttb_indices(&time, &values, 10);
        assert_eq!(indices, vec![0, 1]);
    }

    #[test]
    fn test_lttb_threshold_less_than_3_returns_all() {
        // threshold < 3 returns all indices
        let time: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let values: Vec<f64> = time.iter().map(|t| t.sin()).collect();
        let indices = lttb_indices(&time, &values, 2);
        assert_eq!(indices.len(), 100);
    }

    #[test]
    fn test_lttb_preserves_first_and_last_indices() {
        let time: Vec<f64> = (0..500).map(|i| i as f64).collect();
        let values: Vec<f64> = time.iter().map(|t| (t * 0.05).sin()).collect();
        let indices = lttb_indices(&time, &values, 50);
        assert_eq!(indices[0], 0);
        assert_eq!(*indices.last().unwrap(), 499);
    }

    #[test]
    fn test_lttb_indices_are_sorted() {
        let time: Vec<f64> = (0..1000).map(|i| i as f64).collect();
        let values: Vec<f64> = time.iter().map(|t| (t * 0.01).sin()).collect();
        let indices = lttb_indices(&time, &values, 100);
        for i in 1..indices.len() {
            assert!(
                indices[i] > indices[i - 1],
                "Indices must be strictly increasing"
            );
        }
    }

    #[test]
    fn test_lttb_indices_within_bounds() {
        let n = 2000;
        let time: Vec<f64> = (0..n).map(|i| i as f64).collect();
        let values: Vec<f64> = time.iter().map(|t| (t * 0.1).cos()).collect();
        let indices = lttb_indices(&time, &values, 200);
        for &idx in &indices {
            assert!(idx < n as usize, "Index {} out of bounds (n={})", idx, n);
        }
    }
}
