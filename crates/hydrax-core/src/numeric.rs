use crate::WaveformError;

/// Convert a time-series index to `f64`.
///
/// A vector large enough to exceed `f64`'s exact integer range cannot be
/// allocated on supported targets, so time-series indices remain exact.
#[must_use]
pub fn index_as_f64(index: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let value = index as f64;
    value
}

/// Accept a finite parameter value.
///
/// # Errors
///
/// Returns [`WaveformError::NonFiniteParameter`] for infinities and NaN.
pub fn validate_finite(name: &'static str, value: f64) -> Result<(), WaveformError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(WaveformError::NonFiniteParameter { name, value })
    }
}

/// Accept a finite, strictly positive parameter value.
///
/// # Errors
///
/// Returns [`WaveformError::NonFiniteParameter`] for infinities and NaN, and
/// [`WaveformError::NonPositiveParameter`] for zero or negative values.
pub fn validate_positive(name: &'static str, value: f64) -> Result<(), WaveformError> {
    validate_finite(name, value)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(WaveformError::NonPositiveParameter { name, value })
    }
}

#[cfg(test)]
mod tests {
    use super::{index_as_f64, validate_finite, validate_positive};
    use crate::WaveformError;

    #[test]
    fn index_conversion_is_exact_for_representable_indices() {
        // Expected values are written as exact literals so the assertion does
        // not repeat the conversion it is meant to check.
        for (index, expected) in [
            (0_usize, 0.0_f64),
            (1, 1.0),
            (4096, 4096.0),
            (9_007_199_254_740_991, 9_007_199_254_740_991.0),
        ] {
            assert!((index_as_f64(index) - expected).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn non_finite_values_are_rejected() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let error = validate_finite("test", value).expect_err("non-finite must be rejected");
            assert!(matches!(error, WaveformError::NonFiniteParameter { .. }));
        }
    }

    #[test]
    fn zero_and_negative_values_are_rejected_as_non_positive() {
        for value in [0.0, -0.0, -1.0] {
            let error = validate_positive("test", value).expect_err("value must be rejected");
            assert_eq!(
                error,
                WaveformError::NonPositiveParameter {
                    name: "test",
                    value
                }
            );
        }
    }
}
