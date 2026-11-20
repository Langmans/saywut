//! Signal level for the dB meter.

/// Floor reported for silence or an empty buffer, so the meter never sees -inf.
pub const MIN_DBFS: f32 = -120.0;

/// RMS level of `samples` (range -1.0..=1.0) in dBFS, clamped to [`MIN_DBFS`].
pub fn rms_dbfs(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return MIN_DBFS;
    }
    let mean_square = samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32;
    // 10·log10(mean square) equals 20·log10(rms) without the square root.
    (10.0 * mean_square.log10()).max(MIN_DBFS)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 0.01, "{actual} != {expected}");
    }

    #[test]
    fn empty_buffer_is_floor() {
        assert_eq!(rms_dbfs(&[]), MIN_DBFS);
    }

    #[test]
    fn silence_is_floor() {
        assert_eq!(rms_dbfs(&[0.0; 480]), MIN_DBFS);
    }

    #[test]
    fn full_scale_square_is_zero_db() {
        approx(rms_dbfs(&[1.0, -1.0, 1.0, -1.0]), 0.0);
    }

    #[test]
    fn half_amplitude_is_minus_six_db() {
        approx(rms_dbfs(&[0.5, -0.5, 0.5, -0.5]), -6.02);
    }

    #[test]
    fn tiny_signal_clamps_to_floor() {
        assert_eq!(rms_dbfs(&[1e-9; 16]), MIN_DBFS);
    }
}
