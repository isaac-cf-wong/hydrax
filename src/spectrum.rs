use rustfft::{FftPlanner, num_complex::Complex64};

use crate::{Waveform, WaveformError};

/// A window applied before transforming a finite waveform.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Window {
    /// Leave every sample unchanged.
    Rectangular,
    /// Apply the symmetric Hann window.
    Hann,
    /// Apply a symmetric Tukey window with taper fraction `alpha`.
    ///
    /// `alpha = 0` is rectangular and `alpha = 1` is Hann.
    Tukey {
        /// Fraction of the window occupied by the cosine-tapered regions.
        alpha: f64,
    },
}

impl Window {
    /// Return the mean window weight for a sequence length.
    ///
    /// This coherent gain is useful when recovering a sinusoid's amplitude
    /// from a windowed spectrum. Hydrax does not divide by it automatically.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty sequence or an invalid Tukey fraction.
    pub fn coherent_gain(self, sample_count: usize) -> Result<f64, WaveformError> {
        self.validate()?;
        if sample_count == 0 {
            return Err(WaveformError::TooFewSamples {
                required: 1,
                actual: 0,
            });
        }
        let sum = (0..sample_count)
            .map(|index| self.weight(index, sample_count))
            .sum::<f64>();
        Ok(sum / crate::quadrupole::index_as_f64(sample_count))
    }

    fn validate(self) -> Result<(), WaveformError> {
        if let Self::Tukey { alpha } = self {
            if !alpha.is_finite() {
                return Err(WaveformError::NonFiniteParameter {
                    name: "Tukey alpha",
                    value: alpha,
                });
            }
            if !(0.0..=1.0).contains(&alpha) {
                return Err(WaveformError::ParameterOutOfRange {
                    name: "Tukey alpha",
                    value: alpha,
                    minimum: 0.0,
                    maximum: 1.0,
                });
            }
        }
        Ok(())
    }

    fn weight(self, index: usize, sample_count: usize) -> f64 {
        if sample_count == 1 {
            return 1.0;
        }
        let denominator = crate::quadrupole::index_as_f64(sample_count - 1);
        let position = crate::quadrupole::index_as_f64(index) / denominator;
        match self {
            Self::Rectangular | Self::Tukey { alpha: 0.0 } => 1.0,
            Self::Hann => 0.5 * (1.0 - (2.0 * std::f64::consts::PI * position).cos()),
            Self::Tukey { alpha: 1.0 } => {
                0.5 * (1.0 - (2.0 * std::f64::consts::PI * position).cos())
            }
            Self::Tukey { alpha } if position < alpha / 2.0 => {
                0.5 * (1.0 + (std::f64::consts::PI * (2.0 * position / alpha - 1.0)).cos())
            }
            Self::Tukey { alpha } if position <= 1.0 - alpha / 2.0 => 1.0,
            Self::Tukey { alpha } => {
                0.5 * (1.0
                    + (std::f64::consts::PI * (2.0 * position / alpha - 2.0 / alpha + 1.0)).cos())
            }
        }
    }
}

/// A complex Fourier amplitude with units of seconds for dimensionless strain.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ComplexAmplitude {
    real_s: f64,
    imaginary_s: f64,
}

impl ComplexAmplitude {
    fn from_complex(value: Complex64) -> Self {
        Self {
            real_s: value.re,
            imaginary_s: value.im,
        }
    }

    /// Real component in seconds.
    #[must_use]
    pub const fn real_s(self) -> f64 {
        self.real_s
    }

    /// Imaginary component in seconds.
    #[must_use]
    pub const fn imaginary_s(self) -> f64 {
        self.imaginary_s
    }

    /// Magnitude in seconds.
    #[must_use]
    pub fn magnitude_s(self) -> f64 {
        self.real_s.hypot(self.imaginary_s)
    }

    /// Phase in radians in the interval `[-π, π]`.
    #[must_use]
    pub fn phase_rad(self) -> f64 {
        self.imaginary_s.atan2(self.real_s)
    }
}

/// Nonnegative-frequency Fourier amplitudes of both GW polarizations.
///
/// The transform approximates
/// \(\widetilde h(f)=\int h(t)e^{-2\pi i f t}\,dt\). Values are the
/// nonnegative half of the two-sided transform and are not doubled or divided
/// by the window's coherent gain.
#[derive(Debug, Clone, PartialEq)]
pub struct WaveformSpectrum {
    frequency_resolution_hz: f64,
    h_plus: Vec<ComplexAmplitude>,
    h_cross: Vec<ComplexAmplitude>,
    window: Window,
    coherent_gain: f64,
}

impl WaveformSpectrum {
    /// Separation between adjacent frequency bins in hertz.
    #[must_use]
    pub const fn frequency_resolution_hz(&self) -> f64 {
        self.frequency_resolution_hz
    }

    /// Return the frequency of a bin in hertz.
    #[must_use]
    pub fn frequency_hz(&self, index: usize) -> Option<f64> {
        (index < self.h_plus.len())
            .then_some(crate::quadrupole::index_as_f64(index) * self.frequency_resolution_hz)
    }

    /// Plus-polarization Fourier amplitudes.
    #[must_use]
    pub fn h_plus(&self) -> &[ComplexAmplitude] {
        &self.h_plus
    }

    /// Cross-polarization Fourier amplitudes.
    #[must_use]
    pub fn h_cross(&self) -> &[ComplexAmplitude] {
        &self.h_cross
    }

    /// Window applied before the transform.
    #[must_use]
    pub const fn window(&self) -> Window {
        self.window
    }

    /// Mean weight of the applied window.
    #[must_use]
    pub const fn coherent_gain(&self) -> f64 {
        self.coherent_gain
    }
}

pub(crate) fn spectrum(
    waveform: &Waveform,
    window: Window,
) -> Result<WaveformSpectrum, WaveformError> {
    transform(
        waveform.start_time_s(),
        waveform.sample_interval_s(),
        waveform.h_plus(),
        waveform.h_cross(),
        window,
    )
}

fn transform(
    start_time_s: f64,
    sample_interval_s: f64,
    h_plus: &[f64],
    h_cross: &[f64],
    window: Window,
) -> Result<WaveformSpectrum, WaveformError> {
    let sample_count = h_plus.len();
    if sample_count == 0 {
        return Err(WaveformError::TooFewSamples {
            required: 1,
            actual: 0,
        });
    }
    window.validate()?;
    let coherent_gain = window.coherent_gain(sample_count)?;
    let mut plus_buffer = Vec::with_capacity(sample_count);
    let mut cross_buffer = Vec::with_capacity(sample_count);
    for (index, (plus, cross)) in h_plus.iter().zip(h_cross).enumerate() {
        let weight = window.weight(index, sample_count);
        plus_buffer.push(Complex64::new(weight * plus, 0.0));
        cross_buffer.push(Complex64::new(weight * cross, 0.0));
    }

    let mut planner = FftPlanner::<f64>::new();
    let fft = planner.plan_fft_forward(sample_count);
    fft.process(&mut plus_buffer);
    fft.process(&mut cross_buffer);

    let frequency_resolution_hz =
        1.0 / (crate::quadrupole::index_as_f64(sample_count) * sample_interval_s);
    let bin_count = sample_count / 2 + 1;
    let convert = |(index, value): (usize, &Complex64)| {
        let frequency_hz = crate::quadrupole::index_as_f64(index) * frequency_resolution_hz;
        let phase = Complex64::from_polar(
            1.0,
            -2.0 * std::f64::consts::PI * frequency_hz * start_time_s,
        );
        ComplexAmplitude::from_complex(*value * sample_interval_s * phase)
    };
    let h_plus = plus_buffer
        .iter()
        .take(bin_count)
        .enumerate()
        .map(convert)
        .collect();
    let h_cross = cross_buffer
        .iter()
        .take(bin_count)
        .enumerate()
        .map(convert)
        .collect();

    Ok(WaveformSpectrum {
        frequency_resolution_hz,
        h_plus,
        h_cross,
        window,
        coherent_gain,
    })
}

#[cfg(test)]
mod tests {
    use super::{Window, transform};
    use crate::WaveformError;

    #[test]
    fn constant_signal_has_expected_dc_integral() {
        let samples = vec![2.5; 16];
        let spectrum =
            transform(0.0, 0.25, &samples, &samples, Window::Rectangular).expect("valid input");
        assert!((spectrum.h_plus()[0].real_s() - 10.0).abs() < 1.0e-13);
        assert!(spectrum.h_plus()[0].imaginary_s().abs() < 1.0e-13);
        for amplitude in &spectrum.h_plus()[1..] {
            assert!(amplitude.magnitude_s() < 1.0e-13);
        }
    }

    #[test]
    fn exact_bin_cosine_has_expected_two_sided_amplitude() {
        let sample_count = 64;
        let sample_interval = 0.01;
        let bin = 5;
        let samples = (0..sample_count)
            .map(|index| {
                let phase =
                    2.0 * std::f64::consts::PI * crate::quadrupole::index_as_f64(bin * index)
                        / crate::quadrupole::index_as_f64(sample_count);
                3.0 * phase.cos()
            })
            .collect::<Vec<_>>();
        let zeros = vec![0.0; sample_count];
        let spectrum = transform(0.0, sample_interval, &samples, &zeros, Window::Rectangular)
            .expect("valid input");
        let expected = 3.0 * crate::quadrupole::index_as_f64(sample_count) * sample_interval / 2.0;

        assert!((spectrum.h_plus()[bin].real_s() - expected).abs() < 1.0e-13);
        assert!(spectrum.h_plus()[bin].imaginary_s().abs() < 1.0e-13);
        assert!((spectrum.frequency_hz(bin).expect("valid bin") - 7.8125).abs() < 1.0e-13);
    }

    #[test]
    fn absolute_start_time_rotates_spectral_phase() {
        let samples = (0..32)
            .map(|index| {
                (2.0 * std::f64::consts::PI * crate::quadrupole::index_as_f64(index) / 32.0).cos()
            })
            .collect::<Vec<_>>();
        let zeros = vec![0.0; samples.len()];
        let unshifted =
            transform(0.0, 0.1, &samples, &zeros, Window::Rectangular).expect("valid input");
        let shifted =
            transform(0.8, 0.1, &samples, &zeros, Window::Rectangular).expect("valid input");

        assert!(
            (unshifted.h_plus()[1].magnitude_s() - shifted.h_plus()[1].magnitude_s()).abs()
                < 1.0e-13
        );
        assert!((unshifted.h_plus()[1].phase_rad() - 0.0).abs() < 1.0e-13);
        assert!((shifted.h_plus()[1].phase_rad() + std::f64::consts::FRAC_PI_2).abs() < 1.0e-13);
    }

    #[test]
    fn tukey_limits_match_rectangular_and_hann() {
        for index in 0..17 {
            assert!((Window::Tukey { alpha: 0.0 }.weight(index, 17) - 1.0).abs() < f64::EPSILON);
            assert!(
                (Window::Tukey { alpha: 1.0 }.weight(index, 17) - Window::Hann.weight(index, 17))
                    .abs()
                    < f64::EPSILON
            );
        }
    }

    #[test]
    fn invalid_tukey_fraction_is_rejected() {
        let error = Window::Tukey { alpha: 1.1 }
            .coherent_gain(8)
            .expect_err("invalid alpha");
        assert!(matches!(error, WaveformError::ParameterOutOfRange { .. }));
    }
}
