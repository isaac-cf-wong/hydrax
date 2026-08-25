use hydrax_core::{
    GRAVITATIONAL_CONSTANT_M3_KG_S2, SPEED_OF_LIGHT_M_S, SymmetricTensor3, WaveformError,
    index_as_f64,
};

use crate::QuadrupoleSeries;

/// Centered finite-difference stencil for a third time derivative.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThirdDerivativeStencil {
    /// Five-point centered stencil with second-order truncation error.
    FivePoint,
    /// Seven-point centered stencil with fourth-order truncation error.
    SevenPoint,
}

impl ThirdDerivativeStencil {
    const fn radius(self) -> usize {
        match self {
            Self::FivePoint => 2,
            Self::SevenPoint => 3,
        }
    }

    const fn minimum_samples_for_energy(self) -> usize {
        2 * self.radius() + 2
    }
}

/// Gravitational-wave power samples and their time-integrated energy.
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyDiagnostics {
    start_time_s: f64,
    sample_interval_s: f64,
    power_w: Vec<f64>,
    total_energy_j: f64,
}

impl EnergyDiagnostics {
    /// Time of the first power sample in seconds.
    #[must_use]
    pub const fn start_time_s(&self) -> f64 {
        self.start_time_s
    }

    /// Uniform interval between power samples in seconds.
    #[must_use]
    pub const fn sample_interval_s(&self) -> f64 {
        self.sample_interval_s
    }

    /// Instantaneous quadrupole luminosity in watts.
    #[must_use]
    pub fn power_w(&self) -> &[f64] {
        &self.power_w
    }

    /// Time associated with a power sample.
    #[must_use]
    pub fn time_s(&self, index: usize) -> Option<f64> {
        (index < self.power_w.len())
            .then_some(self.start_time_s + index_as_f64(index) * self.sample_interval_s)
    }

    /// Total radiated energy from trapezoidal integration, in joules.
    #[must_use]
    pub const fn total_energy_j(&self) -> f64 {
        self.total_energy_j
    }

    /// Mean power over the retained diagnostic interval, in watts.
    #[must_use]
    pub fn average_power_w(&self) -> f64 {
        self.total_energy_j / (index_as_f64(self.power_w.len() - 1) * self.sample_interval_s)
    }
}

/// Compute leading-order quadrupole GW luminosity and radiated energy.
///
/// Hydrax evaluates
/// \(P=G\dddot Q_{ij}\dddot Q_{ij}/(5c^5)\) after projecting every
/// numerical derivative to its symmetric trace-free part. The total energy is
/// the trapezoidal integral of the retained power samples.
///
/// # Errors
///
/// Returns an error unless the source contains at least two valid derivative
/// samples for the selected stencil.
pub fn radiated_energy(
    quadrupole: &QuadrupoleSeries,
    stencil: ThirdDerivativeStencil,
) -> Result<EnergyDiagnostics, WaveformError> {
    let required = stencil.minimum_samples_for_energy();
    if quadrupole.samples().len() < required {
        return Err(WaveformError::TooFewSamples {
            required,
            actual: quadrupole.samples().len(),
        });
    }

    let derivatives = third_derivative(quadrupole, stencil);
    let factor = GRAVITATIONAL_CONSTANT_M3_KG_S2 / (5.0 * SPEED_OF_LIGHT_M_S.powi(5));
    let power_w = derivatives
        .iter()
        .map(|derivative| factor * derivative.norm_squared())
        .collect::<Vec<_>>();
    let total_energy_j = power_w
        .windows(2)
        .map(|pair| f64::midpoint(pair[0], pair[1]) * quadrupole.sample_interval_s())
        .sum();

    Ok(EnergyDiagnostics {
        start_time_s: quadrupole.start_time_s()
            + index_as_f64(stencil.radius()) * quadrupole.sample_interval_s(),
        sample_interval_s: quadrupole.sample_interval_s(),
        power_w,
        total_energy_j,
    })
}

fn third_derivative(
    series: &QuadrupoleSeries,
    stencil: ThirdDerivativeStencil,
) -> Vec<SymmetricTensor3> {
    let radius = stencil.radius();
    let dt_cubed = series.sample_interval_s().powi(3);
    (radius..series.samples().len() - radius)
        .map(|index| {
            let derivative = match stencil {
                ThirdDerivativeStencil::FivePoint => {
                    (series.samples()[index - 2] * -1.0 + series.samples()[index - 1] * 2.0
                        - series.samples()[index + 1] * 2.0
                        + series.samples()[index + 2])
                        / (2.0 * dt_cubed)
                }
                ThirdDerivativeStencil::SevenPoint => {
                    (series.samples()[index - 3] - series.samples()[index - 2] * 8.0
                        + series.samples()[index - 1] * 13.0
                        - series.samples()[index + 1] * 13.0
                        + series.samples()[index + 2] * 8.0
                        - series.samples()[index + 3])
                        / (8.0 * dt_cubed)
                }
            };
            derivative.trace_free()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{ThirdDerivativeStencil, radiated_energy, third_derivative};
    use crate::{QuadrupoleSeries, sinusoidal_quadrupole};
    use hydrax_core::{
        GRAVITATIONAL_CONSTANT_M3_KG_S2, SPEED_OF_LIGHT_M_S, SymmetricTensor3, index_as_f64,
    };
    use std::f64::consts::PI;

    /// Largest power or energy, as a fraction of what the isotropic part of the
    /// source would radiate if it were kept, that round-off alone can produce.
    /// The trace-free projection makes the measured fraction zero, while
    /// dropping it makes it unity. The bound has to be relative: a spherical
    /// pulsation of a realistic quadrupole amplitude radiates far below one
    /// watt either way, so an absolute bound in watts holds without the
    /// projection.
    const TRACE_REMOVAL_TOLERANCE: f64 = 1.0e-11;

    /// A spherical pulsation radiates no energy.
    #[test]
    fn isotropic_motion_radiates_no_energy() {
        let amplitude = 1.0e38;
        let omega = 2.0 * PI * 500.0;
        let sample_interval = 1.0 / 32_768.0;
        let samples = (0..64)
            .map(|index| {
                let phase = omega * index_as_f64(index) * sample_interval;
                SymmetricTensor3::isotropic(amplitude * phase.cos())
            })
            .collect();
        let source = QuadrupoleSeries::new(0.0, sample_interval, samples).expect("valid source");
        let diagnostics =
            radiated_energy(&source, ThirdDerivativeStencil::SevenPoint).expect("enough samples");

        // The isotropic third derivative contracts with itself three times over,
        // so this is the power the pulsation radiates without the projection.
        let luminosity_factor =
            GRAVITATIONAL_CONSTANT_M3_KG_S2 / (5.0 * SPEED_OF_LIGHT_M_S.powi(5));
        let unremoved_w = luminosity_factor * 3.0 * (amplitude * omega.powi(3)).powi(2);
        let duration_s = index_as_f64(diagnostics.power_w().len() - 1) * sample_interval;

        for power_w in diagnostics.power_w() {
            let fraction = power_w.abs() / unremoved_w;
            assert!(
                fraction < TRACE_REMOVAL_TOLERANCE,
                "power fraction {fraction}"
            );
        }
        let fraction = diagnostics.total_energy_j().abs() / (unremoved_w * duration_s);
        assert!(
            fraction < TRACE_REMOVAL_TOLERANCE,
            "energy fraction {fraction}"
        );
    }

    #[test]
    fn seven_point_stencil_is_exact_for_cubic() {
        let samples = (0..12)
            .map(|index| {
                let time = index_as_f64(index) * 0.1;
                SymmetricTensor3::new(time.powi(3), 0.0, 0.0, 0.0, 0.0, 0.0)
            })
            .collect();
        let source = QuadrupoleSeries::new(0.0, 0.1, samples).expect("valid source");
        let derivative = third_derivative(&source, ThirdDerivativeStencil::SevenPoint);

        for sample in derivative {
            let [xx, yy, zz, xy, xz, yz] = sample.components();
            assert!((xx - 4.0).abs() < 1.0e-11);
            assert!((yy + 2.0).abs() < 1.0e-11);
            assert!((zz + 2.0).abs() < 1.0e-11);
            assert!(xy.abs() < f64::EPSILON);
            assert!(xz.abs() < f64::EPSILON);
            assert!(yz.abs() < f64::EPSILON);
        }
    }

    #[test]
    fn sinusoidal_energy_matches_analytic_integral() {
        let amplitude = 1.0e38;
        let omega = 2.0 * std::f64::consts::PI * 400.0;
        let sample_interval = 1.0 / 65_536.0;
        let source = sinusoidal_quadrupole(amplitude, omega, 0.0, sample_interval, 4097)
            .expect("valid source");
        let diagnostics =
            radiated_energy(&source, ThirdDerivativeStencil::SevenPoint).expect("enough samples");
        let start = diagnostics.start_time_s();
        let end = diagnostics
            .time_s(diagnostics.power_w().len() - 1)
            .expect("valid final sample");
        let power_scale = 2.0 * GRAVITATIONAL_CONSTANT_M3_KG_S2 * amplitude.powi(2) * omega.powi(6)
            / (5.0 * SPEED_OF_LIGHT_M_S.powi(5));
        let expected = power_scale
            * ((end - start) / 2.0
                - ((2.0 * omega * end).sin() - (2.0 * omega * start).sin()) / (4.0 * omega));
        let relative_error = (diagnostics.total_energy_j() - expected).abs() / expected;

        assert!(relative_error < 2.0e-4, "relative error: {relative_error}");
    }

    #[test]
    fn isotropic_trace_does_not_change_energy() {
        let source = sinusoidal_quadrupole(1.0e38, 2000.0, 0.0, 1.0e-5, 64).expect("valid source");
        let traced_samples = source
            .samples()
            .iter()
            .scan(0.0_f64, |phase, sample| {
                let traced = *sample + SymmetricTensor3::isotropic(1.0e38 * phase.cos());
                *phase += 0.2;
                Some(traced)
            })
            .collect();
        let traced = QuadrupoleSeries::new(
            source.start_time_s(),
            source.sample_interval_s(),
            traced_samples,
        )
        .expect("valid source");
        let expected =
            radiated_energy(&source, ThirdDerivativeStencil::SevenPoint).expect("enough samples");
        let actual =
            radiated_energy(&traced, ThirdDerivativeStencil::SevenPoint).expect("enough samples");

        let relative_error =
            (actual.total_energy_j() - expected.total_energy_j()).abs() / expected.total_energy_j();
        assert!(relative_error < 1.0e-10, "relative error: {relative_error}");
    }

    #[test]
    fn third_derivative_stencils_have_advertised_order() {
        fn maximum_error(
            sample_interval: f64,
            sample_count: usize,
            stencil: ThirdDerivativeStencil,
        ) -> f64 {
            let omega = 2.3;
            let source = sinusoidal_quadrupole(1.0, omega, 0.0, sample_interval, sample_count)
                .expect("valid source");
            let radius = stencil.radius();
            third_derivative(&source, stencil)
                .iter()
                .enumerate()
                .map(|(index, sample)| {
                    let source_index = index + radius;
                    let time = source.time_s(source_index).expect("valid source index");
                    let expected = omega.powi(3) * (omega * time).sin();
                    (sample.components()[0] - expected).abs()
                })
                .fold(0.0, f64::max)
        }

        let five_coarse = maximum_error(0.02, 101, ThirdDerivativeStencil::FivePoint);
        let five_fine = maximum_error(0.01, 201, ThirdDerivativeStencil::FivePoint);
        assert!(five_coarse / five_fine > 3.9);

        let seven_coarse = maximum_error(0.02, 101, ThirdDerivativeStencil::SevenPoint);
        let seven_fine = maximum_error(0.01, 201, ThirdDerivativeStencil::SevenPoint);
        assert!(seven_coarse / seven_fine > 15.0);
    }
}
