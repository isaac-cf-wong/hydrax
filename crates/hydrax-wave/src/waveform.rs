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
    use super::{Observer, Waveform, waveform_from_quadrupole};
    use crate::{FiniteDifference, QuadrupoleSeries, sinusoidal_quadrupole};
    use hydrax_core::{
        GRAVITATIONAL_CONSTANT_M3_KG_S2, SPEED_OF_LIGHT_M_S, SymmetricTensor3, index_as_f64,
    };
    use std::f64::consts::PI;

    const DISTANCE_M: f64 = 3.0e20;

    /// Rotations covering a full turn in both directions. The quarter turn
    /// exchanges the two polarizations and the half turn restores them, so a
    /// wrong factor in front of the angle cannot pass unnoticed.
    const ROTATIONS_RAD: [f64; 12] = [
        0.0,
        0.1,
        PI / 6.0,
        PI / 4.0,
        PI / 3.0,
        PI / 2.0,
        2.0 * PI / 3.0,
        PI,
        -0.7,
        -PI / 4.0,
        3.0 * PI,
        5.5,
    ];

    /// Largest deviation from the 2ψ mixing law, as a fraction of the reference
    /// waveform's peak strain, that round-off alone can produce. Measured
    /// round-off stays below `1.0e-14`, while a basis that does not rotate
    /// correctly about the line of sight deviates by order unity.
    const MIXING_TOLERANCE: f64 = 1.0e-13;

    /// Largest residual, as a fraction of the strain the isotropic part of the
    /// source would produce on its own, that round-off alone can produce.
    /// Measured round-off stays below `4.0e-14`, while an extraction that keeps
    /// any of that part leaves a residual of order unity. The bound has to be
    /// relative: the strains here are of order `1.0e-19`, so any absolute bound
    /// loose enough to be safe is also loose enough to admit the whole signal.
    const TRACE_REMOVAL_TOLERANCE: f64 = 1.0e-11;

    /// Largest departure from exact inverse-distance scaling, as a fraction of
    /// the nearer waveform's peak strain, that round-off alone can produce.
    /// Doubling the distance halves the strain exactly in binary, so the
    /// measured departure is zero; any other power of the distance departs by
    /// order unity.
    const DISTANCE_SCALING_TOLERANCE: f64 = 1.0e-14;

    fn maximum_absolute(values: &[f64]) -> f64 {
        values.iter().copied().map(f64::abs).fold(0.0, f64::max)
    }

    /// The `G/c⁴D` prefactor of the quadrupole formula.
    fn strain_scale(distance_m: f64) -> f64 {
        GRAVITATIONAL_CONSTANT_M3_KG_S2 / (SPEED_OF_LIGHT_M_S.powi(4) * distance_m)
    }

    fn dot(left: [f64; 3], right: [f64; 3]) -> f64 {
        left[0] * right[0] + left[1] * right[1] + left[2] * right[2]
    }

    fn cross(left: [f64; 3], right: [f64; 3]) -> [f64; 3] {
        [
            left[1] * right[2] - left[2] * right[1],
            left[2] * right[0] - left[0] * right[2],
            left[0] * right[1] - left[1] * right[0],
        ]
    }

    /// The unit direction from the source toward the observer.
    fn line_of_sight(observer: Observer) -> [f64; 3] {
        let (sin_theta, cos_theta) = observer.inclination_rad().sin_cos();
        let (sin_phi, cos_phi) = observer.azimuth_rad().sin_cos();
        [sin_theta * cos_phi, sin_theta * sin_phi, cos_theta]
    }

    /// Rows of the matrix that rotates a vector by `angle_rad` about the unit
    /// vector `axis`, following Rodrigues' formula.
    fn rotation_rows(axis: [f64; 3], angle_rad: f64) -> [[f64; 3]; 3] {
        let (sin, cos) = angle_rad.sin_cos();
        let versine = 1.0 - cos;
        let [x, y, z] = axis;
        [
            [
                cos + versine * x * x,
                versine * x * y - sin * z,
                versine * x * z + sin * y,
            ],
            [
                versine * y * x + sin * z,
                cos + versine * y * y,
                versine * y * z - sin * x,
            ],
            [
                versine * z * x - sin * y,
                versine * z * y + sin * x,
                cos + versine * z * z,
            ],
        ]
    }

    /// Rotate every quadrupole sample by `angle_rad` about `axis`, forming
    /// `R Q Rᵀ`.
    fn rotate_source(
        source: &QuadrupoleSeries,
        axis: [f64; 3],
        angle_rad: f64,
    ) -> QuadrupoleSeries {
        let rows = rotation_rows(axis, angle_rad);
        let samples = source
            .samples()
            .iter()
            .map(|sample| {
                SymmetricTensor3::new(
                    sample.contract(rows[0], rows[0]),
                    sample.contract(rows[1], rows[1]),
                    sample.contract(rows[2], rows[2]),
                    sample.contract(rows[0], rows[1]),
                    sample.contract(rows[0], rows[2]),
                    sample.contract(rows[1], rows[2]),
                )
            })
            .collect();
        QuadrupoleSeries::new(source.start_time_s(), source.sample_interval_s(), samples)
            .expect("valid source")
    }

    /// An arbitrary spherical pulsation, with the peak magnitude of its second
    /// derivative in kg m² s⁻². Two incommensurate tones keep the pulsation
    /// from being a single frequency whose cancellation could be accidental.
    fn spherical_pulsation() -> (QuadrupoleSeries, f64) {
        let amplitude = 1.0e38;
        let omega = 2.0 * PI * 700.0;
        let sample_interval = 1.0 / 32_768.0;
        let mut peak_second_derivative = 0.0_f64;
        let samples = (0..256)
            .map(|index| {
                let phase = omega * index_as_f64(index) * sample_interval;
                let overtone = 2.3 * phase + 0.9;
                let radial = amplitude * (phase.cos() + 0.4 * overtone.sin());
                let second_derivative = -amplitude
                    * omega.powi(2)
                    * (phase.cos() + 0.4 * 2.3_f64.powi(2) * overtone.sin());
                peak_second_derivative = peak_second_derivative.max(second_derivative.abs());
                SymmetricTensor3::isotropic(radial)
            })
            .collect();
        let series = QuadrupoleSeries::new(0.0, sample_interval, samples).expect("valid source");
        (series, peak_second_derivative)
    }

    /// A source with all six components active and mutually offset in phase, so
    /// no observer sees a waveform that a rotation could leave unchanged.
    fn anisotropic_source() -> QuadrupoleSeries {
        let amplitude = 1.0e38;
        let omega = 2.0 * PI * 2000.0;
        let sample_interval = 1.0 / 32_768.0;
        let samples = (0..256)
            .map(|index| {
                let phase = omega * index_as_f64(index) * sample_interval;
                SymmetricTensor3::new(
                    amplitude * phase.cos(),
                    -0.4 * amplitude * (phase + 0.9).cos(),
                    0.7 * amplitude * (phase - 1.7).cos(),
                    0.6 * amplitude * (phase - 0.5).sin(),
                    0.3 * amplitude * phase.sin(),
                    -0.2 * amplitude * (phase + 1.3).cos(),
                )
            })
            .collect();
        QuadrupoleSeries::new(0.0, sample_interval, samples).expect("valid source")
    }

    /// Assert that both polarizations of `reference` carry comparable strain, so
    /// that mixing them is observable.
    fn assert_both_polarizations_present(reference: &Waveform) {
        let peak = maximum_absolute(reference.h_plus()).max(maximum_absolute(reference.h_cross()));
        assert!(maximum_absolute(reference.h_plus()) > 0.2 * peak);
        assert!(maximum_absolute(reference.h_cross()) > 0.2 * peak);
    }

    /// Assert that `rotated` holds the polarizations of `reference` mixed by a
    /// rotation through twice `rotation_rad`.
    fn assert_polarizations_mixed_by_twice(
        reference: &Waveform,
        rotated: &Waveform,
        rotation_rad: f64,
    ) {
        let peak = maximum_absolute(reference.h_plus()).max(maximum_absolute(reference.h_cross()));
        let (sin_double, cos_double) = (2.0 * rotation_rad).sin_cos();
        assert_eq!(reference.h_plus().len(), rotated.h_plus().len());

        let expected = reference.h_plus().iter().zip(reference.h_cross());
        let actual = rotated.h_plus().iter().zip(rotated.h_cross());
        for ((&plus, &cross), (&rotated_plus, &rotated_cross)) in expected.zip(actual) {
            let plus_error = (rotated_plus - (cos_double * plus + sin_double * cross)).abs() / peak;
            let cross_error =
                (rotated_cross - (cos_double * cross - sin_double * plus)).abs() / peak;
            assert!(
                plus_error < MIXING_TOLERANCE,
                "rotation {rotation_rad} rad: plus error {plus_error}"
            );
            assert!(
                cross_error < MIXING_TOLERANCE,
                "rotation {rotation_rad} rad: cross error {cross_error}"
            );
        }
    }

    /// An arbitrary spherical pulsation radiates no strain.
    ///
    /// The bound is a fraction of the strain the pulsation would produce if the
    /// two contractions of the plus polarization did not cancel, so an
    /// extraction that keeps part of the isotropic source fails it. Bounding
    /// the strain by an absolute constant instead does not test anything: at
    /// astrophysical distances the whole signal is far below any constant loose
    /// enough to be safe.
    #[test]
    fn spherical_motion_produces_zero_strain() {
        let (source, peak_second_derivative) = spherical_pulsation();
        let unremoved = 2.0 * strain_scale(DISTANCE_M) * peak_second_derivative;

        for (inclination_rad, azimuth_rad) in [(0.0, 0.0), (0.7, -0.4), (1.9, 2.3), (2.9, 0.15)] {
            let observer =
                Observer::new(DISTANCE_M, inclination_rad, azimuth_rad).expect("valid observer");
            let waveform = waveform_from_quadrupole(&source, observer, FiniteDifference::FivePoint)
                .expect("enough samples");
            let angles = format!("inclination {inclination_rad}, azimuth {azimuth_rad}");

            let plus = maximum_absolute(waveform.h_plus()) / unremoved;
            let cross = maximum_absolute(waveform.h_cross()) / unremoved;
            assert!(
                plus < TRACE_REMOVAL_TOLERANCE,
                "plus residual {plus} of the unremoved strain at {angles}"
            );
            assert!(
                cross < TRACE_REMOVAL_TOLERANCE,
                "cross residual {cross} of the unremoved strain at {angles}"
            );
        }
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
        let scale = strain_scale(DISTANCE_M);
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

    /// Both polarizations scale as the inverse of the distance.
    ///
    /// The observer is off both the pole and the diagonal so that neither
    /// polarization vanishes, and the departure is measured against the nearer
    /// waveform's peak strain rather than an absolute constant.
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
        assert_both_polarizations_present(&near);
        let peak = maximum_absolute(near.h_plus()).max(maximum_absolute(near.h_cross()));

        for (near_value, far_value) in near.h_plus().iter().zip(far.h_plus()) {
            let error = (near_value - 2.0 * far_value).abs() / peak;
            assert!(error < DISTANCE_SCALING_TOLERANCE, "plus error {error}");
        }
        for (near_value, far_value) in near.h_cross().iter().zip(far.h_cross()) {
            let error = (near_value - 2.0 * far_value).abs() / peak;
            assert!(error < DISTANCE_SCALING_TOLERANCE, "cross error {error}");
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
        let scale = strain_scale(DISTANCE_M);
        let peak_amplitude = 2.0 * scale * amplitude * omega.powi(2);

        assert!(maximum_absolute(waveform.h_plus()) < 1.0e-30);
        for (index, value) in waveform.h_cross().iter().enumerate() {
            let time = waveform.time_s(index).expect("valid waveform index");
            let expected = -peak_amplitude * (omega * time).cos();
            let normalized_error = (value - expected).abs() / peak_amplitude;
            assert!(normalized_error < 3.0e-6);
        }
    }

    /// Adding an isotropic trace to the source leaves both polarizations alone.
    ///
    /// The added trace is three times the source amplitude and oscillates at
    /// the same frequency, so the strain it would contribute is larger than the
    /// waveform itself, and the difference is measured as a fraction of that
    /// contribution.
    #[test]
    fn isotropic_trace_does_not_change_waveform() {
        let amplitude = 1.0e38;
        let omega = 2.0 * PI * 600.0;
        let sample_interval = 1.0 / 32_768.0;
        let source = sinusoidal_quadrupole(amplitude, omega, 0.0, sample_interval, 256)
            .expect("valid source");
        let trace_amplitude = 3.0 * amplitude;
        let traced_samples = source
            .samples()
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                let phase = omega * index_as_f64(index) * sample_interval + 0.7;
                *sample + SymmetricTensor3::isotropic(trace_amplitude * phase.cos())
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
        assert_both_polarizations_present(&expected);
        let unremoved = 2.0 * strain_scale(DISTANCE_M) * trace_amplitude * omega.powi(2);

        for (expected, actual) in expected.h_plus().iter().zip(actual.h_plus()) {
            let error = (expected - actual).abs() / unremoved;
            assert!(error < TRACE_REMOVAL_TOLERANCE, "plus error {error}");
        }
        for (expected, actual) in expected.h_cross().iter().zip(actual.h_cross()) {
            let error = (expected - actual).abs() / unremoved;
            assert!(error < TRACE_REMOVAL_TOLERANCE, "cross error {error}");
        }
    }

    /// An observer rotation about the line of sight transforms the two
    /// polarizations correctly.
    ///
    /// At zero inclination the line of sight is the z-axis whatever the azimuth,
    /// so advancing the azimuth by ψ leaves the observer where it is and turns
    /// its transverse basis `(p, q)` through ψ about the line of sight. The
    /// polarizations must then mix as a rotation through 2ψ: a quarter turn
    /// exchanges them and a half turn restores them.
    #[test]
    fn observer_rotation_about_line_of_sight_mixes_polarizations_by_twice_the_angle() {
        let source = anisotropic_source();
        let reference = waveform_from_quadrupole(
            &source,
            Observer::new(DISTANCE_M, 0.0, 0.0).expect("valid observer"),
            FiniteDifference::FivePoint,
        )
        .expect("enough samples");
        assert_both_polarizations_present(&reference);

        for rotation_rad in ROTATIONS_RAD {
            let rotated = waveform_from_quadrupole(
                &source,
                Observer::new(DISTANCE_M, 0.0, rotation_rad).expect("valid observer"),
                FiniteDifference::FivePoint,
            )
            .expect("enough samples");
            assert_polarizations_mixed_by_twice(&reference, &rotated, rotation_rad);
        }
    }

    /// The same requirement away from the pole.
    ///
    /// Only at the pole does the azimuth turn the observer's basis about the
    /// line of sight, so at a general sky position the rotation is applied to
    /// the source instead: turning the source through −ψ about the line of
    /// sight describes the same physical configuration as turning the observer's
    /// basis through +ψ about it, and must mix the polarizations by 2ψ. Unlike
    /// the pole, this exercises the inclination-dependent components of `p`.
    #[test]
    fn observer_rotation_mixes_polarizations_at_a_general_sky_position() {
        let source = anisotropic_source();
        for (inclination_rad, azimuth_rad) in [(0.7, -0.4), (1.9, 2.3), (2.9, 0.15)] {
            let observer =
                Observer::new(DISTANCE_M, inclination_rad, azimuth_rad).expect("valid observer");
            let axis = line_of_sight(observer);
            let reference =
                waveform_from_quadrupole(&source, observer, FiniteDifference::FivePoint)
                    .expect("enough samples");
            assert_both_polarizations_present(&reference);

            for rotation_rad in ROTATIONS_RAD {
                let rotated_source = rotate_source(&source, axis, -rotation_rad);
                let rotated = waveform_from_quadrupole(
                    &rotated_source,
                    observer,
                    FiniteDifference::FivePoint,
                )
                .expect("enough samples");
                assert_polarizations_mixed_by_twice(&reference, &rotated, rotation_rad);
            }
        }
    }

    /// The 2ψ mixing law holds only because `(p, q, n)` is a right-handed
    /// orthonormal triad whose first two vectors are transverse to the line of
    /// sight `n`. Sweeping both angles past the poles and beyond a full turn
    /// covers the whole domain the constructor accepts.
    #[test]
    fn polarization_basis_is_a_right_handed_transverse_triad() {
        for inclination_step in -4_i32..=12 {
            for azimuth_step in -6_i32..=6 {
                let inclination_rad = f64::from(inclination_step) * PI / 6.0;
                let azimuth_rad = f64::from(azimuth_step) * PI / 4.0;
                let observer = Observer::new(DISTANCE_M, inclination_rad, azimuth_rad)
                    .expect("valid observer");
                let (p, q) = observer.polarization_basis();
                let n = line_of_sight(observer);
                let angles = format!("inclination {inclination_rad}, azimuth {azimuth_rad}");

                assert!(
                    (dot(p, p) - 1.0).abs() < 1.0e-14,
                    "p is not a unit vector: {angles}"
                );
                assert!(
                    (dot(q, q) - 1.0).abs() < 1.0e-14,
                    "q is not a unit vector: {angles}"
                );
                assert!(
                    dot(p, q).abs() < 1.0e-14,
                    "p and q are not orthogonal: {angles}"
                );
                assert!(dot(p, n).abs() < 1.0e-14, "p is not transverse: {angles}");
                assert!(dot(q, n).abs() < 1.0e-14, "q is not transverse: {angles}");
                for (component, expected) in cross(p, q).into_iter().zip(n) {
                    assert!(
                        (component - expected).abs() < 1.0e-14,
                        "p × q is not the line of sight: {angles}"
                    );
                }
            }
        }
    }
}
