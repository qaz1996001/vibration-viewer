use serde::{Deserialize, Serialize};

/// FFT spectrum data for a single time point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpectrumData {
    pub frequencies: Vec<f64>,
    pub amplitudes: Vec<f64>,
    pub sample_rate: f64,
    pub fft_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spectrum_data_serialization_roundtrip() {
        let spectrum = SpectrumData {
            frequencies: vec![0.0, 50.0, 100.0, 150.0, 200.0],
            amplitudes: vec![0.1, 0.8, 0.3, 0.05, 0.02],
            sample_rate: 1000.0,
            fft_size: 1024,
        };

        let json = serde_json::to_string(&spectrum).unwrap();
        let deserialized: SpectrumData = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.frequencies.len(), 5);
        assert_eq!(deserialized.amplitudes.len(), 5);
        assert!((deserialized.frequencies[1] - 50.0).abs() < 1e-6);
        assert!((deserialized.amplitudes[1] - 0.8).abs() < 1e-6);
        assert!((deserialized.sample_rate - 1000.0).abs() < 1e-6);
        assert_eq!(deserialized.fft_size, 1024);
    }
}
