//! Convergence tests of the waveform pipeline against its analytic solution.
//!
//! Hydrax's quadrupole formula is exact; everything between a source and a
//! strain is a discretization of it. These tests refine the sample interval and
//! measure the rate at which the computed answer approaches the closed-form
//! one, which is the only check that distinguishes a stencil that is merely
//! plausible from one that is the order it advertises.
//!
//! They run against the `hydrax` facade rather than a layer, so what is
//! measured is the accuracy a caller actually gets.
//!
//! # Reading an observed order
//!
//! On sample intervals that are halved in turn, an error that scales as `dt^p`
//! falls by `2^p` per refinement, so `log2(E_coarse / E_fine)` estimates `p`.
//! The assertions bound that estimate from *both* sides. A lower bound alone
//! passes a measurement that has stopped tracking the discretization at all --
//! an error compared against the wrong times, a norm over a shrinking window,
//! an error that has already fallen to round-off -- because each of those makes
//! the ratio larger, not smaller. Requiring the observed order to sit in a
//! narrow band around the designed order is what makes these tests able to
//! fail.
//!
//! # Why the refinements stop where they do
//!
//! A centered second-derivative stencil divides by `dt^2`, so its round-off
//! *grows* as the grid refines while its truncation error shrinks. The
//! sequences here stay in the range where truncation dominates round-off by
//! several orders of magnitude; refining much further would measure the
//! floating-point format instead of the scheme, and the observed order would
//! fall away from its designed value rather than approach it.

use hydrax::{
    EnergyDiagnostics, FiniteDifference, GRAVITATIONAL_CONSTANT_M3_KG_S2, Observer,
    QuadrupoleSeries, SPEED_OF_LIGHT_M_S, ThirdDerivativeStencil, Waveform, WaveformError,
    kiloparsecs_to_m, radiated_energy, sinusoidal_quadrupole, waveform_from_quadrupole,
};

/// Number of grids in a refinement sequence.
///
/// Four grids give three independent order estimates, which distinguishes a
/// scheme converging at its designed rate from one that happens to halve its
/// error once.
const REFINEMENTS: usize = 4;

/// Half-width of the band an observed order must fall in.
///
/// Wide enough for the subleading terms of a truncation series that has not
/// quite reached its asymptotic limit, and far narrower than the gap between
/// any two integer orders.
const ORDER_TOLERANCE: f64 = 0.15;

/// Quadrupole amplitude of the test source, in kg m^2.
const AMPLITUDE_KG_M2: f64 = 1.0e38;

/// Angular frequency of the test source, in rad/s: 800 Hz, in the band a
/// core-collapse supernova radiates in.
const ANGULAR_FREQUENCY_RAD_S: f64 = 2.0 * std::f64::consts::PI * 800.0;

/// Coarsest sample interval of the refinement sequences, in seconds.
const COARSEST_SAMPLE_INTERVAL_S: f64 = 1.0 / 16_384.0;

/// Number of sample intervals the coarsest grid spans.
///
/// The product with the coarsest interval fixes the physical duration every
/// refinement covers: about 8 ms, or six and a half cycles of the source. It is
/// deliberately not a whole number of half-cycles, so that the endpoint terms
/// of the trapezoidal energy integral do not cancel and hide its second-order
/// error behind the fourth-order one of the stencil.
const COARSEST_INTERVAL_COUNT: usize = 131;

/// Refinements skipped before the radiated-energy sequence starts.
///
/// The energy is a trapezoidal integral of the power samples, so its error is
/// the sum of a second-order quadrature term and the stencil's own. With the
/// seven-point stencil the fourth-order stencil term is still the larger of the
/// two on the grids the other measurements use, and the sequence there reads an
/// order between three and two while the crossover happens. Starting three
/// refinements finer puts the whole sequence past that crossover, where the
/// quadrature term is what remains and the order is the flat two the trapezoid
/// rule promises.
const ENERGY_REFINEMENT_OFFSET: usize = 3;

/// Observer inclination, in radians: away from both the pole and the equator,
/// so the inclination-dependent part of the polarization basis contributes.
const INCLINATION_RAD: f64 = 0.9;

/// Observer azimuth, in radians: chosen with the inclination so that neither
/// polarization vanishes and each carries a comparable share of the strain.
const AZIMUTH_RAD: f64 = 0.6;

/// Convert a small count to `f64`.
///
/// The counts here are a few thousand at most, so the conversion is exact.
fn as_f64(count: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let value = count as f64;
    value
}

/// Estimate the order of accuracy between each pair of successive grids.
fn observed_orders(errors: &[f64]) -> Vec<f64> {
    errors
        .windows(2)
        .map(|pair| (pair[0] / pair[1]).log2())
        .collect()
}

/// Require every observed order to lie within [`ORDER_TOLERANCE`] of `designed`.
fn assert_designed_order(label: &str, errors: &[f64], designed: f64) {
    assert!(
        errors.iter().all(|error| error.is_finite() && *error > 0.0),
        "{label}: errors must be finite and nonzero to estimate an order: {errors:?}"
    );
    for (refinement, order) in observed_orders(errors).iter().enumerate() {
        assert!(
            (order - designed).abs() < ORDER_TOLERANCE,
            "{label}: refinement {refinement} converged at order {order}, not {designed} \
             (errors {errors:?})"
        );
    }
}

/// The observer every waveform measurement uses.
fn observer() -> Observer {
    Observer::new(kiloparsecs_to_m(10.0), INCLINATION_RAD, AZIMUTH_RAD)
        .expect("a positive distance and finite angles")
}

/// The `G / (c^4 D)` prefactor of the quadrupole formula.
fn strain_scale() -> f64 {
    GRAVITATIONAL_CONSTANT_M3_KG_S2 / (SPEED_OF_LIGHT_M_S.powi(4) * observer().distance_m())
}

/// The analytic polarizations of the sinusoidal source at time `t`.
///
/// For `Q = A cos(w t) diag(1, -1, 0)` the quadrupole formula gives
/// `h+ = -(G A w^2 / c^4 D) (1 + cos^2 i) cos(2 phi) cos(w t)` and
/// `hx = 2 (G A w^2 / c^4 D) cos(i) sin(2 phi) cos(w t)`. Both are worked out
/// by hand from the projection onto the transverse basis; Hydrax computes
/// neither, which is what makes them an anchor rather than a restatement.
fn analytic_polarizations(time_s: f64) -> (f64, f64) {
    let peak = strain_scale() * AMPLITUDE_KG_M2 * ANGULAR_FREQUENCY_RAD_S.powi(2);
    let phase = (ANGULAR_FREQUENCY_RAD_S * time_s).cos();
    let inclination = INCLINATION_RAD.cos();
    (
        -peak * (1.0 + inclination * inclination) * (2.0 * AZIMUTH_RAD).cos() * phase,
        2.0 * peak * inclination * (2.0 * AZIMUTH_RAD).sin() * phase,
    )
}

/// The analytic quadrupole luminosity at time `t`, in watts.
///
/// The source is already trace free, so its third derivative is
/// `A w^3 sin(w t) diag(1, -1, 0)`, whose contraction with itself is
/// `2 A^2 w^6 sin^2(w t)`, and `P = G <...Q ...Q> / 5 c^5`.
fn analytic_power_w(time_s: f64) -> f64 {
    let factor = GRAVITATIONAL_CONSTANT_M3_KG_S2 / (5.0 * SPEED_OF_LIGHT_M_S.powi(5));
    let sine = (ANGULAR_FREQUENCY_RAD_S * time_s).sin();
    2.0 * factor * AMPLITUDE_KG_M2.powi(2) * ANGULAR_FREQUENCY_RAD_S.powi(6) * sine * sine
}

/// The analytic radiated energy over `[0, duration]`, in joules.
///
/// `integral of sin^2(w t) dt` from zero is `t / 2 - sin(2 w t) / 4 w`.
fn analytic_energy_j(duration_s: f64) -> f64 {
    let factor = GRAVITATIONAL_CONSTANT_M3_KG_S2 / (5.0 * SPEED_OF_LIGHT_M_S.powi(5));
    let amplitude = 2.0 * factor * AMPLITUDE_KG_M2.powi(2) * ANGULAR_FREQUENCY_RAD_S.powi(6);
    amplitude
        * (duration_s / 2.0
            - (2.0 * ANGULAR_FREQUENCY_RAD_S * duration_s).sin() / (4.0 * ANGULAR_FREQUENCY_RAD_S))
}

/// Sample interval and number of sample intervals of refinement `refinement`.
fn grid(refinement: usize) -> (f64, usize) {
    let steps = 1_usize << refinement;
    (
        COARSEST_SAMPLE_INTERVAL_S / as_f64(steps),
        COARSEST_INTERVAL_COUNT * steps,
    )
}

/// A source sampled so that the diagnostic it feeds starts at `t = 0` and spans
/// exactly `intervals` steps of `sample_interval_s`.
///
/// Centered stencils discard `radius` samples at each end, so the source is
/// extended by that much on both sides. Without the padding the retained
/// interval would move by `radius * dt` under refinement, and an integral over
/// it would converge at the rate at which its own limits stopped moving rather
/// than at the rate of the scheme.
fn padded_source(
    sample_interval_s: f64,
    intervals: usize,
    radius: usize,
) -> Result<QuadrupoleSeries, WaveformError> {
    sinusoidal_quadrupole(
        AMPLITUDE_KG_M2,
        ANGULAR_FREQUENCY_RAD_S,
        -as_f64(radius) * sample_interval_s,
        sample_interval_s,
        intervals + 1 + 2 * radius,
    )
}

/// The number of samples `difference` discards at each end.
///
/// Measured through the public API rather than assumed, so a stencil whose
/// width changes cannot silently unpad the sources above.
fn waveform_radius(difference: FiniteDifference) -> usize {
    const SAMPLES: usize = 16;
    let source = sinusoidal_quadrupole(1.0, 1.0, 0.0, 1.0, SAMPLES).expect("a valid source");
    let waveform =
        waveform_from_quadrupole(&source, observer(), difference).expect("enough samples");
    (SAMPLES - waveform.h_plus().len()) / 2
}

/// The number of samples `stencil` discards at each end.
fn energy_radius(stencil: ThirdDerivativeStencil) -> usize {
    const SAMPLES: usize = 16;
    let source = sinusoidal_quadrupole(1.0, 1.0, 0.0, 1.0, SAMPLES).expect("a valid source");
    let energy = radiated_energy(&source, stencil).expect("enough samples");
    (SAMPLES - energy.power_w().len()) / 2
}

/// Assert that a diagnostic starts where the padding was chosen to put it.
///
/// The padding is what keeps the compared interval fixed across refinements;
/// if it were off by a sample the errors below would still shrink, just at the
/// wrong rate and for the wrong reason.
fn assert_starts_at_origin(start_time_s: f64, sample_interval_s: f64) {
    assert!(
        start_time_s.abs() < 1.0e-9 * sample_interval_s,
        "padding left the diagnostic starting at {start_time_s} s"
    );
}

/// The largest error in each polarization on the grid of `refinement`,
/// as a fraction of that polarization's analytic peak.
fn waveform_errors(difference: FiniteDifference, refinement: usize) -> (f64, f64) {
    let (sample_interval_s, intervals) = grid(refinement);
    let source = padded_source(sample_interval_s, intervals, waveform_radius(difference))
        .expect("a valid source");
    let waveform =
        waveform_from_quadrupole(&source, observer(), difference).expect("enough samples");
    assert_starts_at_origin(waveform.start_time_s(), sample_interval_s);
    assert_eq!(waveform.h_plus().len(), intervals + 1);

    normalized_errors(&waveform)
}

/// Compare a waveform against the analytic polarizations sample by sample.
fn normalized_errors(waveform: &Waveform) -> (f64, f64) {
    let (mut plus_peak, mut cross_peak) = (0.0_f64, 0.0_f64);
    let (mut plus_error, mut cross_error) = (0.0_f64, 0.0_f64);
    for (index, (&plus, &cross)) in waveform.h_plus().iter().zip(waveform.h_cross()).enumerate() {
        let time_s = waveform.time_s(index).expect("a valid sample index");
        let (expected_plus, expected_cross) = analytic_polarizations(time_s);
        plus_peak = plus_peak.max(expected_plus.abs());
        cross_peak = cross_peak.max(expected_cross.abs());
        plus_error = plus_error.max((plus - expected_plus).abs());
        cross_error = cross_error.max((cross - expected_cross).abs());
    }
    assert!(
        plus_peak > 0.0 && cross_peak > 0.0,
        "the observer must see both polarizations for the comparison to mean anything"
    );
    (plus_error / plus_peak, cross_error / cross_peak)
}

/// The largest power error on the grid of `refinement`, as a fraction of the
/// analytic peak luminosity, together with the error in the total energy.
fn energy_errors(stencil: ThirdDerivativeStencil, refinement: usize) -> (f64, f64) {
    let (sample_interval_s, intervals) = grid(refinement);
    let source = padded_source(sample_interval_s, intervals, energy_radius(stencil))
        .expect("a valid source");
    let energy = radiated_energy(&source, stencil).expect("enough samples");
    assert_starts_at_origin(energy.start_time_s(), sample_interval_s);
    assert_eq!(energy.power_w().len(), intervals + 1);

    (
        normalized_power_error(&energy),
        normalized_energy_error(&energy, as_f64(intervals) * sample_interval_s),
    )
}

/// Compare the luminosity samples against the analytic luminosity.
fn normalized_power_error(energy: &EnergyDiagnostics) -> f64 {
    let mut peak = 0.0_f64;
    let mut error = 0.0_f64;
    for (index, &power_w) in energy.power_w().iter().enumerate() {
        let time_s = energy.time_s(index).expect("a valid sample index");
        let expected = analytic_power_w(time_s);
        peak = peak.max(expected);
        error = error.max((power_w - expected).abs());
    }
    error / peak
}

/// Compare the integrated energy against the analytic integral.
fn normalized_energy_error(energy: &EnergyDiagnostics, duration_s: f64) -> f64 {
    let expected = analytic_energy_j(duration_s);
    assert!(expected > 0.0, "the source must radiate over the interval");
    (energy.total_energy_j() - expected).abs() / expected
}

/// Both polarizations converge to the analytic waveform at the order of the
/// stencil that produced them.
///
/// The two stencils are checked in the same test because the pipeline around
/// them is identical: what changes between the two measurements is the order,
/// and nothing else. Each polarization is measured separately, since a basis
/// error that mixed them would leave the total strain converging while one
/// component did not.
#[test]
fn the_waveform_converges_at_the_order_of_its_stencil() {
    for (difference, designed) in [
        (FiniteDifference::ThreePoint, 2.0),
        (FiniteDifference::FivePoint, 4.0),
    ] {
        let measured = (0..REFINEMENTS)
            .map(|refinement| waveform_errors(difference, refinement))
            .collect::<Vec<_>>();
        let plus = measured.iter().map(|pair| pair.0).collect::<Vec<_>>();
        let cross = measured.iter().map(|pair| pair.1).collect::<Vec<_>>();
        assert_designed_order(&format!("{difference:?} h_plus"), &plus, designed);
        assert_designed_order(&format!("{difference:?} h_cross"), &cross, designed);
    }
}

/// The luminosity converges to the analytic quadrupole power at the order of
/// its third-derivative stencil.
#[test]
fn the_luminosity_converges_at_the_order_of_its_stencil() {
    for (stencil, designed) in [
        (ThirdDerivativeStencil::FivePoint, 2.0),
        (ThirdDerivativeStencil::SevenPoint, 4.0),
    ] {
        let errors = (0..REFINEMENTS)
            .map(|refinement| energy_errors(stencil, refinement).0)
            .collect::<Vec<_>>();
        assert_designed_order(&format!("{stencil:?} power"), &errors, designed);
    }
}

/// The total radiated energy converges at second order whichever stencil
/// produced the luminosity.
///
/// The energy is a trapezoidal integral of the power samples, and the trapezoid
/// rule is second-order accurate. With the five-point stencil that matches the
/// power it integrates; with the seven-point stencil it does not, and the
/// quadrature -- not the derivative -- is what limits the answer. Asserting the
/// same order for both is the statement of that: an implementation that
/// silently gained fourth-order energy from a fourth-order stencil would have
/// to have changed the quadrature, and this test would notice.
#[test]
fn the_radiated_energy_converges_at_the_order_of_its_quadrature() {
    for stencil in [
        ThirdDerivativeStencil::FivePoint,
        ThirdDerivativeStencil::SevenPoint,
    ] {
        let errors = (0..REFINEMENTS)
            .map(|refinement| energy_errors(stencil, refinement + ENERGY_REFINEMENT_OFFSET).1)
            .collect::<Vec<_>>();
        assert_designed_order(&format!("{stencil:?} energy"), &errors, 2.0);
    }
}
