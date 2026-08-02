use hydrax_core::{
    GRAVITATIONAL_CONSTANT_M3_KG_S2, SPEED_OF_LIGHT_M_S, WaveformError, index_as_f64,
    validate_finite, validate_positive,
};

use crate::{FiniteDifference, QuadrupoleSeries};

/// Location and orientation of a distant observer.
///
/// Inclination is measured from the positive Cartesian z-axis. Azimuth is
/// measured from the positive x-axis toward the positive y-axis. Angles use
/// radians and distance uses metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Observer {
    distance_m: f64,
    inclination_rad: f64,
    azimuth_rad: f64,
}

impl Observer {
    /// Construct and validate an observer.
    ///
    /// # Errors
    ///
    /// Returns an error unless distance is finite and positive and both angles
    /// are finite.
    pub fn new(
        distance_m: f64,
        inclination_rad: f64,
        azimuth_rad: f64,
    ) -> Result<Self, WaveformError> {
        validate_positive("observer distance", distance_m)?;
        validate_finite("observer inclination", inclination_rad)?;
        validate_finite("observer azimuth", azimuth_rad)?;
        Ok(Self {
            distance_m,
            inclination_rad,
            azimuth_rad,
        })
    }

    /// Source distance in metres.
    #[must_use]
    pub const fn distance_m(self) -> f64 {
        self.distance_m
    }

    /// Observer inclination in radians.
    #[must_use]
    pub const fn inclination_rad(self) -> f64 {
        self.inclination_rad
    }

    /// Observer azimuth in radians.
    #[must_use]
    pub const fn azimuth_rad(self) -> f64 {
        self.azimuth_rad
    }

    fn polarization_basis(self) -> ([f64; 3], [f64; 3]) {
        let (sin_theta, cos_theta) = self.inclination_rad.sin_cos();
        let (sin_phi, cos_phi) = self.azimuth_rad.sin_cos();
        let p = [cos_theta * cos_phi, cos_theta * sin_phi, -sin_theta];
        let q = [-sin_phi, cos_phi, 0.0];
        (p, q)
    }
}

/// The two dimensionless gravitational-wave polarizations.
#[derive(Debug, Clone, PartialEq)]
pub struct Waveform {
    start_time_s: f64,
    sample_interval_s: f64,
    h_plus: Vec<f64>,
    h_cross: Vec<f64>,
}

impl Waveform {
    /// Time of the first waveform sample in seconds.
    #[must_use]
    pub const fn start_time_s(&self) -> f64 {
        self.start_time_s
    }

    /// Uniform interval between waveform samples in seconds.
    #[must_use]
    pub const fn sample_interval_s(&self) -> f64 {
        self.sample_interval_s
    }

    /// Plus-polarization strain samples.
    #[must_use]
    pub fn h_plus(&self) -> &[f64] {
        &self.h_plus
    }

    /// Cross-polarization strain samples.
    #[must_use]
    pub fn h_cross(&self) -> &[f64] {
        &self.h_cross
    }

    /// Return the time associated with a waveform sample index.
    #[must_use]
    pub fn time_s(&self, index: usize) -> Option<f64> {
        (index < self.h_plus.len())
            .then_some(self.start_time_s + index_as_f64(index) * self.sample_interval_s)
    }

    /// Transform both polarizations to nonnegative frequencies.
    ///
    /// # Errors
    ///
    /// Returns an error if the selected window has invalid parameters.
    pub fn spectrum(
        &self,
        window: crate::Window,
    ) -> Result<crate::WaveformSpectrum, WaveformError> {
        crate::spectrum::spectrum(self, window)
    }
}

/// Compute the leading-order gravitational waveform from a mass quadrupole.
///
/// This applies the slow-motion, weak-field quadrupole formula
/// \(h_{ij}^{TT}=2G\ddot Q_{ij}^{TT}/(c^4D)\). The returned times use the
/// source's retarded-time coordinate; the constant propagation delay `D/c` is
/// not added. Centered stencils discard their boundary radius, so the waveform
/// has two or four fewer samples than the input.
///
/// # Errors
///
/// Returns an error when the input does not contain enough samples for the
/// selected finite-difference stencil.
pub fn waveform_from_quadrupole(
    quadrupole: &QuadrupoleSeries,
    observer: Observer,
    difference: FiniteDifference,
) -> Result<Waveform, WaveformError> {
    let second_derivative = crate::quadrupole::second_derivative(quadrupole, difference)?;
    let (p, q) = observer.polarization_basis();
    let scale =
        GRAVITATIONAL_CONSTANT_M3_KG_S2 / (SPEED_OF_LIGHT_M_S.powi(4) * observer.distance_m);

    let mut h_plus = Vec::with_capacity(second_derivative.samples().len());
    let mut h_cross = Vec::with_capacity(second_derivative.samples().len());
    for sample in second_derivative.samples() {
        h_plus.push(scale * (sample.contract(p, p) - sample.contract(q, q)));
        h_cross.push(2.0 * scale * sample.contract(p, q));
    }

    Ok(Waveform {
        start_time_s: second_derivative.start_time_s(),
        sample_interval_s: second_derivative.sample_interval_s(),
        h_plus,
        h_cross,
    })
}

#[cfg(test)]
mod tests {
    use super::{Observer, waveform_from_quadrupole};
    use crate::{FiniteDifference, QuadrupoleSeries, sinusoidal_quadrupole};
    use hydrax_core::{
        GRAVITATIONAL_CONSTANT_M3_KG_S2, SPEED_OF_LIGHT_M_S, SymmetricTensor3, index_as_f64,
    };

    const DISTANCE_M: f64 = 3.0e20;

    fn maximum_absolute(values: &[f64]) -> f64 {
        values.iter().copied().map(f64::abs).fold(0.0, f64::max)
    }

    #[test]
    fn spherical_motion_produces_zero_strain() {
        let samples = (0..20)
            .map(|index| SymmetricTensor3::isotropic((f64::from(index) * 0.2).sin()))
            .collect();
        let source = QuadrupoleSeries::new(0.0, 0.01, samples).expect("valid source");
        let observer = Observer::new(DISTANCE_M, 0.7, -0.4).expect("valid observer");
        let waveform = waveform_from_quadrupole(&source, observer, FiniteDifference::FivePoint)
            .expect("enough samples");

        assert!(maximum_absolute(waveform.h_plus()) < 1.0e-50);
        assert!(maximum_absolute(waveform.h_cross()) < 1.0e-50);
    }

    #[test]
    fn sinusoidal_plus_matches_analytic_waveform() {
        let amplitude = 1.0e38;
        let omega = 2.0 * std::f64::consts::PI * 800.0;
        let source = sinusoidal_quadrupole(amplitude, omega, 0.0, 1.0 / 65_536.0, 1024)
            .expect("valid source");
        let observer = Observer::new(DISTANCE_M, 0.0, 0.0).expect("valid observer");
        let waveform = waveform_from_quadrupole(&source, observer, FiniteDifference::FivePoint)
            .expect("enough samples");
        let scale = GRAVITATIONAL_CONSTANT_M3_KG_S2 / (SPEED_OF_LIGHT_M_S.powi(4) * DISTANCE_M);
        let peak_amplitude = 2.0 * scale * amplitude * omega.powi(2);

        for (index, value) in waveform.h_plus().iter().enumerate() {
            let time = waveform.time_s(index).expect("valid waveform index");
            let expected = -2.0 * scale * amplitude * omega.powi(2) * (omega * time).cos();
            let normalized_error = (value - expected).abs() / peak_amplitude;
            assert!(
                normalized_error < 3.0e-6,
                "normalized error: {normalized_error}"
            );
        }
        assert!(maximum_absolute(waveform.h_cross()) < 1.0e-30);
    }

    #[test]
    fn strain_scales_inversely_with_distance() {
        let source = sinusoidal_quadrupole(1.0e38, 3000.0, 0.0, 1.0e-5, 32).expect("valid source");
        let near = waveform_from_quadrupole(
            &source,
            Observer::new(DISTANCE_M, 0.3, 0.2).expect("valid observer"),
            FiniteDifference::FivePoint,
        )
        .expect("enough samples");
        let far = waveform_from_quadrupole(
            &source,
            Observer::new(2.0 * DISTANCE_M, 0.3, 0.2).expect("valid observer"),
            FiniteDifference::FivePoint,
        )
        .expect("enough samples");

        for (near_value, far_value) in near.h_plus().iter().zip(far.h_plus()) {
            assert!((near_value - 2.0 * far_value).abs() < 1.0e-35);
        }
    }

    #[test]
    fn off_diagonal_source_produces_cross_polarization() {
        let amplitude = 1.0e38;
        let omega = 2.0 * std::f64::consts::PI * 500.0;
        let sample_interval = 1.0 / 32_768.0;
        let samples = (0..512)
            .map(|index| {
                let time = index_as_f64(index) * sample_interval;
                let moment = amplitude * (omega * time).cos();
                SymmetricTensor3::new(0.0, 0.0, 0.0, moment, 0.0, 0.0)
            })
            .collect();
        let source = QuadrupoleSeries::new(0.0, sample_interval, samples).expect("valid source");
        let observer = Observer::new(DISTANCE_M, 0.0, 0.0).expect("valid observer");
        let waveform = waveform_from_quadrupole(&source, observer, FiniteDifference::FivePoint)
            .expect("enough samples");
        let scale = GRAVITATIONAL_CONSTANT_M3_KG_S2 / (SPEED_OF_LIGHT_M_S.powi(4) * DISTANCE_M);
        let peak_amplitude = 2.0 * scale * amplitude * omega.powi(2);

        assert!(maximum_absolute(waveform.h_plus()) < 1.0e-30);
        for (index, value) in waveform.h_cross().iter().enumerate() {
            let time = waveform.time_s(index).expect("valid waveform index");
            let expected = -peak_amplitude * (omega * time).cos();
            let normalized_error = (value - expected).abs() / peak_amplitude;
            assert!(normalized_error < 3.0e-6);
        }
    }

    #[test]
    fn isotropic_trace_does_not_change_waveform() {
        let source = sinusoidal_quadrupole(2.0, 4.0, 0.0, 0.01, 32).expect("valid source");
        let traced_samples = source
            .samples()
            .iter()
            .scan(0.0_f64, |phase, sample| {
                let traced = *sample + SymmetricTensor3::isotropic(phase.cos());
                *phase += 0.3;
                Some(traced)
            })
            .collect();
        let traced = QuadrupoleSeries::new(
            source.start_time_s(),
            source.sample_interval_s(),
            traced_samples,
        )
        .expect("valid source");
        let observer = Observer::new(DISTANCE_M, 0.6, 1.2).expect("valid observer");
        let expected = waveform_from_quadrupole(&source, observer, FiniteDifference::FivePoint)
            .expect("enough samples");
        let actual = waveform_from_quadrupole(&traced, observer, FiniteDifference::FivePoint)
            .expect("enough samples");

        for (expected, actual) in expected.h_plus().iter().zip(actual.h_plus()) {
            assert!((expected - actual).abs() < 1.0e-45);
        }
        for (expected, actual) in expected.h_cross().iter().zip(actual.h_cross()) {
            assert!((expected - actual).abs() < 1.0e-45);
        }
    }
}
