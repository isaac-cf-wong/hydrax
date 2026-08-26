//! Differential tests of the compiled kernels against their safe-Rust twins.
//!
//! A compiled kernel cannot be trusted on inspection: it is written in another
//! language, compiled with flags the reader did not choose, and will eventually
//! run on a GPU. It can be trusted when it reproduces an implementation short
//! enough to read, over inputs that reach every branch it has. That is what
//! these tests do, and it is how a CUDA or HIP backend of the same ABI will be
//! checked in turn.
//!
//! Agreement is required to a few units in the last place rather than exactly.
//! The two implementations perform the same operations in the same order and
//! the C is compiled without contraction, so in practice they agree bit for
//! bit; the tolerance is there so that a compiler which contracts anyway
//! reports a rounding difference as agreement and an algorithmic difference as
//! a failure. Zero must be reproduced exactly, because the tolerance is
//! relative.

use hydrax_kernels_sys::{
    CONSERVED_COUNT, KernelError, KernelStatus, RiemannStates, SphericalShells, hll_flux,
    minmod_slopes, reference, spherical_flux_divergence,
};

/// Largest relative difference accepted between a kernel and its reference.
///
/// Roughly forty-five units in the last place: far above any difference the
/// order of two identical expressions can produce, and far below the smallest
/// difference a changed algorithm would.
const RELATIVE_TOLERANCE: f64 = 1.0e-14;

/// A deterministic source of test inputs.
///
/// A fixed generator rather than a random one: a differential failure has to
/// be reproducible from the test name alone, and a seed that changes per run
/// turns a real disagreement into a flake.
struct Values {
    /// Current state of the linear congruential generator.
    state: u64,
}

impl Values {
    /// Start a generator from a fixed seed.
    const fn seeded(seed: u64) -> Self {
        Self { state: seed }
    }

    /// Return the next value, uniform on `[-scale, scale)`.
    fn sample(&mut self, scale: f64) -> f64 {
        // The multiplier and increment are Knuth's MMIX constants.
        self.state = self
            .state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let unit = f64::from(u32::try_from(self.state >> 32).expect("a shifted u64 fits a u32"))
            / f64::from(u32::MAX);
        scale * unit.mul_add(2.0, -1.0)
    }

    /// Return `count` values, uniform on `[-scale, scale)`.
    fn sequence(&mut self, count: usize, scale: f64) -> Vec<f64> {
        (0..count).map(|_| self.sample(scale)).collect()
    }
}

/// Assert that two buffers agree to within the relative tolerance.
///
/// Equal values -- two NaNs, two infinities of the same sign, or any exact
/// match -- count as agreement before the tolerance is consulted. A non-finite
/// payload is not rejected by the kernels, so reproducing where it propagates
/// to is part of the contract, and a relative tolerance cannot express that:
/// the difference of two equal infinities is NaN.
#[track_caller]
#[allow(
    clippy::float_cmp,
    reason = "equality is the fast path of the comparison, not an approximation of it"
)]
fn assert_agrees(kernel: &[f64], reference: &[f64], case: &str) {
    assert_eq!(kernel.len(), reference.len(), "{case}: differing lengths");
    for (index, (found, want)) in kernel.iter().zip(reference).enumerate() {
        if found == want || (found.is_nan() && want.is_nan()) {
            continue;
        }
        let difference = (found - want).abs();
        let magnitude = found.abs().max(want.abs());
        assert!(
            difference <= RELATIVE_TOLERANCE * magnitude,
            "{case}: entry {index} is {found} from the kernel and {want} from the reference"
        );
    }
}

/// Run both implementations of the minmod kernel and compare them.
#[track_caller]
fn compare_minmod(values: &[f64], case: &str) {
    let mut from_kernel = vec![f64::NAN; values.len()];
    let mut from_reference = vec![f64::NAN; values.len()];
    let kernel_status = minmod_slopes(values, &mut from_kernel);
    let reference_status = reference::minmod_slopes(values, &mut from_reference);
    assert_eq!(kernel_status, reference_status, "{case}: differing status");
    if kernel_status.is_ok() {
        assert_agrees(&from_kernel, &from_reference, case);
    }
}

/// Run both implementations of the HLL kernel and compare them.
#[track_caller]
fn compare_hll(states: &RiemannStates<'_>, case: &str) {
    let values = states.interface_count() * CONSERVED_COUNT;
    let mut from_kernel = vec![f64::NAN; values];
    let mut from_reference = vec![f64::NAN; values];
    let kernel_status = hll_flux(states, &mut from_kernel);
    let reference_status = reference::hll_flux(states, &mut from_reference);
    assert_eq!(kernel_status, reference_status, "{case}: differing status");
    if kernel_status.is_ok() {
        assert_agrees(&from_kernel, &from_reference, case);
    }
}

/// Run both implementations of the divergence kernel and compare them.
#[track_caller]
fn compare_divergence(shells: &SphericalShells<'_>, case: &str) {
    let values = shells.cell_count() * CONSERVED_COUNT;
    let mut from_kernel = vec![f64::NAN; values];
    let mut from_reference = vec![f64::NAN; values];
    let kernel_status = spherical_flux_divergence(shells, &mut from_kernel);
    let reference_status = reference::spherical_flux_divergence(shells, &mut from_reference);
    assert_eq!(kernel_status, reference_status, "{case}: differing status");
    if kernel_status.is_ok() {
        assert_agrees(&from_kernel, &from_reference, case);
    }
}

#[test]
fn minmod_agrees_over_a_swept_field() {
    let mut values = Values::seeded(0x5EED_0001);
    for cells in 1..=64_usize {
        // A wide scale range so the limiter meets both smooth data and jumps
        // of many orders of magnitude, where the smaller-magnitude branch is
        // the only thing separating the two implementations.
        for scale in [1.0e-30, 1.0, 1.0e12] {
            let field = values.sequence(cells, scale);
            compare_minmod(&field, &format!("{cells} cells at scale {scale}"));
        }
    }
}

#[test]
fn minmod_agrees_on_the_shapes_that_select_each_branch() {
    for (case, field) in [
        ("monotone rise", vec![0.0, 1.0, 3.0, 6.0, 10.0]),
        ("monotone fall", vec![10.0, 6.0, 3.0, 1.0, 0.0]),
        ("maximum", vec![0.0, 1.0, 5.0, 1.0, 0.0]),
        ("minimum", vec![5.0, 1.0, 0.0, 1.0, 5.0]),
        ("flat", vec![2.0; 5]),
        ("equal differences", vec![0.0, 1.0, 2.0, 3.0, 4.0]),
        ("signed zeros", vec![0.0, -0.0, 0.0, -0.0, 0.0]),
        ("single cell", vec![7.0]),
        ("two cells", vec![7.0, -7.0]),
        (
            "infinite neighbour",
            vec![0.0, 1.0, f64::INFINITY, 1.0, 0.0],
        ),
        ("nan neighbour", vec![0.0, 1.0, f64::NAN, 1.0, 0.0]),
        (
            "subnormal differences",
            vec![0.0, f64::MIN_POSITIVE, 2.0 * f64::MIN_POSITIVE, 0.0],
        ),
    ] {
        compare_minmod(&field, case);
    }
}

#[test]
fn hll_agrees_over_swept_states_and_speeds() {
    let mut values = Values::seeded(0x5EED_0002);
    for interfaces in 1..=32_usize {
        let count = interfaces * CONSERVED_COUNT;
        let left_state = values.sequence(count, 1.0e3);
        let right_state = values.sequence(count, 1.0e3);
        let left_flux = values.sequence(count, 1.0e5);
        let right_flux = values.sequence(count, 1.0e5);
        // Speeds drawn as an ordered pair around a random centre, so the fan
        // straddles the interface, sits entirely to one side, or degenerates
        // to a point, in whatever mixture the sweep produces.
        let centres = values.sequence(interfaces, 1.0);
        let widths = values.sequence(interfaces, 1.0);
        let left_speed = centres
            .iter()
            .zip(&widths)
            .map(|(centre, width)| centre - width.abs())
            .collect::<Vec<_>>();
        let right_speed = centres
            .iter()
            .zip(&widths)
            .map(|(centre, width)| centre + width.abs())
            .collect::<Vec<_>>();
        compare_hll(
            &RiemannStates {
                left_state: &left_state,
                right_state: &right_state,
                left_flux: &left_flux,
                right_flux: &right_flux,
                left_speed: &left_speed,
                right_speed: &right_speed,
            },
            &format!("{interfaces} interfaces"),
        );
    }
}

#[test]
fn hll_agrees_on_every_wave_configuration() {
    let left_state = [1.0, 2.0, 3.0];
    let right_state = [1.5, -2.5, 3.5];
    let left_flux = [0.25, -0.5, 0.75];
    let right_flux = [-0.125, 0.625, -1.5];
    for (case, slowest, fastest) in [
        ("supersonic to the right", 0.5, 1.5),
        ("stationary left edge", 0.0, 1.5),
        ("supersonic to the left", -1.5, -0.5),
        ("stationary right edge", -1.5, 0.0),
        ("straddling", -1.0, 1.0),
        ("barely straddling", -f64::MIN_POSITIVE, f64::MIN_POSITIVE),
        ("degenerate at rest", 0.0, 0.0),
        ("nearly degenerate", -1.0, -1.0 + f64::EPSILON),
        ("wide fan", -1.0e8, 1.0e8),
    ] {
        compare_hll(
            &RiemannStates {
                left_state: &left_state,
                right_state: &right_state,
                left_flux: &left_flux,
                right_flux: &right_flux,
                left_speed: &[slowest],
                right_speed: &[fastest],
            },
            case,
        );
    }
}

#[test]
fn hll_agrees_on_a_non_finite_payload() {
    // The kernels validate the speeds and the geometry, not the payload, so a
    // non-finite state has to propagate identically rather than be rejected.
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        compare_hll(
            &RiemannStates {
                left_state: &[value, 1.0, 1.0],
                right_state: &[1.0, value, 1.0],
                left_flux: &[1.0, 1.0, value],
                right_flux: &[value, value, 1.0],
                left_speed: &[-1.0],
                right_speed: &[1.0],
            },
            &format!("payload {value}"),
        );
    }
}

#[test]
#[allow(
    clippy::float_cmp,
    reason = "the output must be untouched, which is exact equality with the fill value"
)]
fn hll_rejects_a_degenerate_speed_pair_identically() {
    let state = [1.0, 2.0, 3.0];
    for (case, slowest, fastest) in [
        ("inverted", 1.0, -1.0),
        ("nan slowest", f64::NAN, 1.0),
        ("nan fastest", -1.0, f64::NAN),
        ("infinite fan", f64::NEG_INFINITY, f64::INFINITY),
        ("infinite slowest", f64::NEG_INFINITY, 1.0),
        ("infinite fastest", -1.0, f64::INFINITY),
    ] {
        let states = RiemannStates {
            left_state: &state,
            right_state: &state,
            left_flux: &state,
            right_flux: &state,
            left_speed: &[slowest],
            right_speed: &[fastest],
        };
        let mut flux = [7.0; CONSERVED_COUNT];
        assert_eq!(
            hll_flux(&states, &mut flux),
            Err(KernelError::Rejected(KernelStatus::DegenerateWaveSpeeds)),
            "{case}"
        );
        // A rejected call leaves the output as it found it, so a caller can
        // correct the input and retry without knowing how far the kernel got.
        assert_eq!(flux, [7.0; CONSERVED_COUNT], "{case}: output was written");
        compare_hll(&states, case);
    }
}

#[test]
fn a_late_degenerate_speed_pair_still_leaves_the_output_untouched() {
    // The rejection is found on the last interface, after every earlier one
    // has passed validation: a kernel that validated and computed in one pass
    // would already have written most of the buffer.
    let interfaces = 8;
    let count = interfaces * CONSERVED_COUNT;
    let payload = vec![1.0; count];
    let mut left_speed = vec![-1.0; interfaces];
    let right_speed = vec![1.0; interfaces];
    left_speed[interfaces - 1] = 2.0;
    let states = RiemannStates {
        left_state: &payload,
        right_state: &payload,
        left_flux: &payload,
        right_flux: &payload,
        left_speed: &left_speed,
        right_speed: &right_speed,
    };
    let mut flux = vec![7.0; count];
    assert_eq!(
        hll_flux(&states, &mut flux),
        Err(KernelError::Rejected(KernelStatus::DegenerateWaveSpeeds))
    );
    assert_eq!(flux, vec![7.0; count]);
    compare_hll(&states, "degenerate final interface");
}

#[test]
fn divergence_agrees_over_a_swept_spherical_grid() {
    let mut values = Values::seeded(0x5EED_0003);
    for cells in 1..=32_usize {
        let interfaces = cells + 1;
        let interface_flux = values.sequence(interfaces * CONSERVED_COUNT, 1.0e4);
        let source = values.sequence(cells * CONSERVED_COUNT, 1.0e2);
        // A geometrically stretched grid, so the areas and volumes span many
        // orders of magnitude the way a collapse grid does.
        let radii = (0..interfaces)
            .map(|face| 1.0e3 * 1.15_f64.powi(i32::try_from(face).expect("a small count")))
            .collect::<Vec<_>>();
        let area = radii
            .iter()
            .map(|radius| 4.0 * std::f64::consts::PI * radius * radius)
            .collect::<Vec<_>>();
        let volume = radii
            .windows(2)
            .map(|shell| 4.0 / 3.0 * std::f64::consts::PI * (shell[1].powi(3) - shell[0].powi(3)))
            .collect::<Vec<_>>();
        compare_divergence(
            &SphericalShells {
                interface_flux: &interface_flux,
                interface_area: &area,
                cell_volume: &volume,
                source: &source,
            },
            &format!("{cells} shells"),
        );
    }
}

#[test]
fn divergence_agrees_at_a_regular_centre() {
    // The innermost interface of a spherical grid has zero area, which is the
    // one geometric value the kernel accepts but cannot divide by.
    let shells = SphericalShells {
        interface_flux: &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0],
        interface_area: &[0.0, 12.566_370_614_359_172, 50.265_482_457_436_69],
        cell_volume: &[4.188_790_204_786_391, 29.321_531_433_504_74],
        source: &[0.1, 0.2, 0.3, 0.4, 0.5, 0.6],
    };
    compare_divergence(&shells, "regular centre");
}

#[test]
fn divergence_agrees_on_a_non_finite_payload() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        compare_divergence(
            &SphericalShells {
                interface_flux: &[value, 1.0, 1.0, 1.0, value, 1.0],
                interface_area: &[1.0, 2.0],
                cell_volume: &[0.5],
                source: &[1.0, 1.0, value],
            },
            &format!("payload {value}"),
        );
    }
}

#[test]
#[allow(
    clippy::float_cmp,
    reason = "the output must be untouched, which is exact equality with the fill value"
)]
fn divergence_rejects_unusable_geometry_identically() {
    let flux = [1.0; 2 * CONSERVED_COUNT];
    let source = [0.0; CONSERVED_COUNT];
    for (case, area, volume) in [
        ("negative area", vec![-1.0, 2.0], vec![0.5]),
        ("nan area", vec![f64::NAN, 2.0], vec![0.5]),
        ("infinite area", vec![1.0, f64::INFINITY], vec![0.5]),
        ("zero volume", vec![1.0, 2.0], vec![0.0]),
        ("negative volume", vec![1.0, 2.0], vec![-0.5]),
        ("nan volume", vec![1.0, 2.0], vec![f64::NAN]),
        ("infinite volume", vec![1.0, 2.0], vec![f64::INFINITY]),
    ] {
        let shells = SphericalShells {
            interface_flux: &flux,
            interface_area: &area,
            cell_volume: &volume,
            source: &source,
        };
        let mut rhs = [7.0; CONSERVED_COUNT];
        assert_eq!(
            spherical_flux_divergence(&shells, &mut rhs),
            Err(KernelError::Rejected(KernelStatus::InvalidGeometry)),
            "{case}"
        );
        assert_eq!(rhs, [7.0; CONSERVED_COUNT], "{case}: output was written");
        compare_divergence(&shells, case);
    }
}

#[test]
fn an_empty_domain_is_rejected_by_both_implementations() {
    let empty: [f64; 0] = [];
    let mut slopes: [f64; 0] = [];
    assert_eq!(
        minmod_slopes(&empty, &mut slopes),
        Err(KernelError::Rejected(KernelStatus::EmptyDomain))
    );
    assert_eq!(
        reference::minmod_slopes(&empty, &mut slopes),
        Err(KernelError::Rejected(KernelStatus::EmptyDomain))
    );

    let states = RiemannStates {
        left_state: &empty,
        right_state: &empty,
        left_flux: &empty,
        right_flux: &empty,
        left_speed: &empty,
        right_speed: &empty,
    };
    let mut flux: [f64; 0] = [];
    assert_eq!(
        hll_flux(&states, &mut flux),
        Err(KernelError::Rejected(KernelStatus::EmptyDomain))
    );
    assert_eq!(
        reference::hll_flux(&states, &mut flux),
        Err(KernelError::Rejected(KernelStatus::EmptyDomain))
    );

    let shells = SphericalShells {
        interface_flux: &[1.0, 2.0, 3.0],
        interface_area: &[1.0],
        cell_volume: &empty,
        source: &empty,
    };
    let mut rhs: [f64; 0] = [];
    assert_eq!(
        spherical_flux_divergence(&shells, &mut rhs),
        Err(KernelError::Rejected(KernelStatus::EmptyDomain))
    );
    assert_eq!(
        reference::spherical_flux_divergence(&shells, &mut rhs),
        Err(KernelError::Rejected(KernelStatus::EmptyDomain))
    );
}

#[test]
fn a_buffer_the_domain_size_does_not_explain_is_named_in_the_error() {
    let field = [1.0, 2.0, 3.0];
    let mut short = [0.0; 2];
    assert_eq!(
        minmod_slopes(&field, &mut short),
        Err(KernelError::LengthMismatch {
            buffer: "slopes",
            expected: 3,
            actual: 2,
        })
    );

    let three = [1.0; CONSERVED_COUNT];
    let two = [1.0; 2];
    let mut flux = [0.0; CONSERVED_COUNT];
    assert_eq!(
        hll_flux(
            &RiemannStates {
                left_state: &two,
                right_state: &three,
                left_flux: &three,
                right_flux: &three,
                left_speed: &[0.0],
                right_speed: &[0.0],
            },
            &mut flux
        ),
        Err(KernelError::LengthMismatch {
            buffer: "left_state",
            expected: CONSERVED_COUNT,
            actual: 2,
        })
    );

    let mut rhs = [0.0; CONSERVED_COUNT];
    assert_eq!(
        spherical_flux_divergence(
            &SphericalShells {
                interface_flux: &[1.0; 2 * CONSERVED_COUNT],
                interface_area: &[1.0],
                cell_volume: &[1.0],
                source: &three,
            },
            &mut rhs
        ),
        Err(KernelError::LengthMismatch {
            buffer: "interface_area",
            expected: 2,
            actual: 1,
        })
    );
}

#[test]
#[allow(
    clippy::float_cmp,
    reason = "every expected value is exactly representable and exactly reached"
)]
fn the_kernels_reproduce_hand_computed_values() {
    // The differential tests establish that the two implementations agree.
    // These establish what they agree on, against values worked out by hand
    // from the definitions rather than from either implementation.
    let mut slopes = [0.0; 5];
    minmod_slopes(&[0.0, 1.0, 3.0, 4.0, 4.0], &mut slopes).expect("a well-formed field");
    // Interior differences are (1, 2), (2, 1) and (1, 0).
    assert_eq!(slopes, [0.0, 1.0, 1.0, 0.0, 0.0]);

    // A stationary contact: equal fluxes and a fan straddling the interface
    // must give back the shared flux plus the jump term.
    let mut flux = [0.0; CONSERVED_COUNT];
    hll_flux(
        &RiemannStates {
            left_state: &[1.0, 0.0, 0.0],
            right_state: &[3.0, 0.0, 0.0],
            left_flux: &[0.0; CONSERVED_COUNT],
            right_flux: &[0.0; CONSERVED_COUNT],
            left_speed: &[-2.0],
            right_speed: &[2.0],
        },
        &mut flux,
    )
    .expect("a well-formed Riemann problem");
    // (2 * 0 - (-2) * 0 + (-2) * 2 * (3 - 1)) / (2 - (-2)) = -2.
    assert_eq!(flux, [-2.0, 0.0, 0.0]);

    // A flux uniform in radius through a spherical shell of growing area.
    let mut rhs = [0.0; CONSERVED_COUNT];
    spherical_flux_divergence(
        &SphericalShells {
            interface_flux: &[1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            interface_area: &[1.0, 3.0],
            cell_volume: &[2.0],
            source: &[0.5, 0.0, 0.0],
        },
        &mut rhs,
    )
    .expect("a well-formed shell");
    // 0.5 - (3 * 1 - 1 * 1) / 2 = -0.5.
    assert_eq!(rhs, [-0.5, 0.0, 0.0]);
}
