use hydrax_core::{
    SymmetricTensor3, WaveformError, index_as_f64, validate_finite, validate_positive,
};

/// A uniformly sampled Cartesian mass-quadrupole time series.
#[derive(Debug, Clone, PartialEq)]
pub struct QuadrupoleSeries {
    start_time_s: f64,
    sample_interval_s: f64,
    samples: Vec<SymmetricTensor3>,
}

impl QuadrupoleSeries {
    /// Construct and validate a quadrupole time series.
    ///
    /// Quadrupole components must use kg m², while time values use seconds.
    ///
    /// # Errors
    ///
    /// Returns an error if either time parameter or any sample is non-finite,
    /// or if the sample interval is not positive.
    pub fn new(
        start_time_s: f64,
        sample_interval_s: f64,
        samples: Vec<SymmetricTensor3>,
    ) -> Result<Self, WaveformError> {
        validate_finite("start time", start_time_s)?;
        validate_positive("sample interval", sample_interval_s)?;
        if let Some(index) = samples.iter().position(|sample| !sample.is_finite()) {
            return Err(WaveformError::NonFiniteSample { index });
        }
        Ok(Self {
            start_time_s,
            sample_interval_s,
            samples,
        })
    }

    /// Time of the first sample in seconds.
    #[must_use]
    pub const fn start_time_s(&self) -> f64 {
        self.start_time_s
    }

    /// Uniform interval between samples in seconds.
    #[must_use]
    pub const fn sample_interval_s(&self) -> f64 {
        self.sample_interval_s
    }

    /// Borrow the quadrupole samples.
    #[must_use]
    pub fn samples(&self) -> &[SymmetricTensor3] {
        &self.samples
    }

    /// Return the time associated with a sample index.
    #[must_use]
    pub fn time_s(&self, index: usize) -> Option<f64> {
        (index < self.samples.len())
            .then_some(self.start_time_s + index_as_f64(index) * self.sample_interval_s)
    }
}

/// Centered finite-difference stencil for the second time derivative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FiniteDifference {
    /// Three-point centered stencil with second-order truncation error.
    ThreePoint,
    /// Five-point centered stencil with fourth-order truncation error.
    FivePoint,
}

impl FiniteDifference {
    pub(crate) const fn radius(self) -> usize {
        match self {
            Self::ThreePoint => 1,
            Self::FivePoint => 2,
        }
    }

    pub(crate) const fn minimum_samples(self) -> usize {
        2 * self.radius() + 1
    }
}

/// Generate a trace-free sinusoidal quadrupole for examples and verification.
///
/// The nonzero components are
/// \(Q_{xx}=A\cos(\omega t)\) and \(Q_{yy}=-A\cos(\omega t)\).
///
/// # Errors
///
/// Returns an error for non-finite parameters, a non-positive sample interval,
/// or an empty sample sequence.
pub fn sinusoidal_quadrupole(
    amplitude_kg_m2: f64,
    angular_frequency_rad_s: f64,
    start_time_s: f64,
    sample_interval_s: f64,
    sample_count: usize,
) -> Result<QuadrupoleSeries, WaveformError> {
    validate_finite("quadrupole amplitude", amplitude_kg_m2)?;
    validate_finite("angular frequency", angular_frequency_rad_s)?;
    if sample_count == 0 {
        return Err(WaveformError::TooFewSamples {
            required: 1,
            actual: 0,
        });
    }
    validate_positive("sample interval", sample_interval_s)?;
    validate_finite("start time", start_time_s)?;

    let samples = (0..sample_count)
        .map(|index| {
            let time = start_time_s + index_as_f64(index) * sample_interval_s;
            let moment = amplitude_kg_m2 * (angular_frequency_rad_s * time).cos();
            SymmetricTensor3::new(moment, -moment, 0.0, 0.0, 0.0, 0.0)
        })
        .collect();
    QuadrupoleSeries::new(start_time_s, sample_interval_s, samples)
}

pub(crate) fn second_derivative(
    series: &QuadrupoleSeries,
    scheme: FiniteDifference,
) -> Result<QuadrupoleSeries, WaveformError> {
    let required = scheme.minimum_samples();
    if series.samples.len() < required {
        return Err(WaveformError::TooFewSamples {
            required,
            actual: series.samples.len(),
        });
    }

    let radius = scheme.radius();
    let dt_squared = series.sample_interval_s * series.sample_interval_s;
    let samples = (radius..series.samples.len() - radius)
        .map(|index| match scheme {
            FiniteDifference::ThreePoint => {
                (series.samples[index - 1] - series.samples[index] * 2.0
                    + series.samples[index + 1])
                    / dt_squared
            }
            FiniteDifference::FivePoint => {
                (series.samples[index - 2] * -1.0 + series.samples[index - 1] * 16.0
                    - series.samples[index] * 30.0
                    + series.samples[index + 1] * 16.0
                    - series.samples[index + 2])
                    / (12.0 * dt_squared)
            }
        })
        .collect();

    QuadrupoleSeries::new(
        series.start_time_s + index_as_f64(radius) * series.sample_interval_s,
        series.sample_interval_s,
        samples,
    )
}

#[cfg(test)]
mod tests {
    use super::{FiniteDifference, QuadrupoleSeries, second_derivative};
    use crate::sinusoidal_quadrupole;
    use hydrax_core::{SymmetricTensor3, WaveformError};

    #[test]
    fn rejects_non_finite_sample() {
        let error = QuadrupoleSeries::new(
            0.0,
            1.0,
            vec![SymmetricTensor3::new(f64::NAN, 0.0, 0.0, 0.0, 0.0, 0.0)],
        )
        .expect_err("NaN must be rejected");
        assert_eq!(error, WaveformError::NonFiniteSample { index: 0 });
    }

    #[test]
    fn three_point_derivative_is_exact_for_quadratic() {
        let samples = (0..7)
            .map(|index| {
                let time = f64::from(index) * 0.1;
                SymmetricTensor3::new(time * time, 0.0, 0.0, 0.0, 0.0, 0.0)
            })
            .collect();
        let series = QuadrupoleSeries::new(0.0, 0.1, samples).expect("valid series");
        let derivative =
            second_derivative(&series, FiniteDifference::ThreePoint).expect("enough samples");
        for sample in derivative.samples() {
            assert!((sample.components()[0] - 2.0).abs() < 1.0e-12);
        }
    }

    #[test]
    fn five_point_derivative_converges_at_fourth_order() {
        fn maximum_error(
            sample_interval: f64,
            sample_count: usize,
            difference: FiniteDifference,
        ) -> f64 {
            let series = sinusoidal_quadrupole(1.0, 2.3, 0.0, sample_interval, sample_count)
                .expect("valid sinusoid");
            let derivative = second_derivative(&series, difference).expect("enough samples");
            derivative
                .samples()
                .iter()
                .enumerate()
                .map(|(index, sample)| {
                    let time = derivative.time_s(index).expect("valid sample index");
                    let expected = -(2.3_f64).powi(2) * (2.3 * time).cos();
                    (sample.components()[0] - expected).abs()
                })
                .fold(0.0, f64::max)
        }

        let coarse = maximum_error(0.02, 101, FiniteDifference::FivePoint);
        let fine = maximum_error(0.01, 201, FiniteDifference::FivePoint);
        assert!(coarse / fine > 15.0, "observed ratio: {}", coarse / fine);
    }

    #[test]
    fn three_point_derivative_converges_at_second_order() {
        fn maximum_error(sample_interval: f64, sample_count: usize) -> f64 {
            let series = sinusoidal_quadrupole(1.0, 2.3, 0.0, sample_interval, sample_count)
                .expect("valid sinusoid");
            let derivative =
                second_derivative(&series, FiniteDifference::ThreePoint).expect("enough samples");
            derivative
                .samples()
                .iter()
                .enumerate()
                .map(|(index, sample)| {
                    let time = derivative.time_s(index).expect("valid sample index");
                    let expected = -(2.3_f64).powi(2) * (2.3 * time).cos();
                    (sample.components()[0] - expected).abs()
                })
                .fold(0.0, f64::max)
        }

        let coarse = maximum_error(0.02, 101);
        let fine = maximum_error(0.01, 201);
        assert!(coarse / fine > 3.9, "observed ratio: {}", coarse / fine);
    }
}
