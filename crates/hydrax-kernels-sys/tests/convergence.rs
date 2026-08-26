//! Convergence tests of the compiled kernels against analytic references.
//!
//! The differential tests establish that the compiled kernels reproduce their
//! safe-Rust twins. That says nothing about whether either one discretizes the
//! operator it claims to: two implementations of the same wrong stencil agree
//! perfectly. These tests supply the missing half by comparing each kernel to
//! a closed-form answer it never sees, on a sequence of refined grids, and
//! measuring the rate at which the difference falls.
//!
//! Every kernel is measured twice, once compiled and once through
//! [`reference`], so a convergence claim covers both implementations rather
//! than resting on the differential tests to carry it across.
//!
//! # Reading an observed order
//!
//! On grids whose spacing is halved, an error that scales as `h^p` falls by
//! `2^p` per refinement, so `log2(E_coarse / E_fine)` estimates `p`. The
//! assertions bound that estimate from *both* sides. A lower bound alone
//! passes a test whose error measure has collapsed to something unrelated to
//! the discretization -- a comparison against the wrong point, a norm taken
//! over an empty range, an error that has already reached round-off -- because
//! all of those make the ratio large rather than small. Requiring the observed
//! order to sit in a narrow band around the designed order is what makes these
//! tests able to fail.

use hydrax_kernels_sys::{
    CONSERVED_COUNT, KernelError, SphericalShells, minmod_slopes, reference,
    spherical_flux_divergence,
};

/// Number of grids in a refinement sequence.
///
/// Four grids give three independent order estimates, which distinguishes a
/// scheme that converges at its designed rate from one that happens to halve
/// its error once.
const REFINEMENTS: usize = 4;

/// Half-width of the band an observed order must fall in.
///
/// The refinement sequences below keep the leading truncation term at least two
/// orders of magnitude above both the next term in its series and the
/// accumulated round-off, so the estimate sits well inside this band; a stencil
/// of the wrong order misses it by a whole integer.
const ORDER_TOLERANCE: f64 = 0.15;

/// Whether a kernel is measured compiled or through its safe-Rust twin.
///
/// Both are held to the same convergence requirement: the reference is what
/// the differential tests define correctness against, so an order it does not
/// meet would make those tests meaningless.
#[derive(Debug, Clone, Copy)]
enum Implementation {
    /// The compiled C kernel reached through the FFI.
    Compiled,
    /// The slow safe-Rust twin in `reference`.
    Reference,
}

impl Implementation {
    /// Both implementations, so every measurement covers each in turn.
    const BOTH: [Self; 2] = [Self::Compiled, Self::Reference];

    fn spherical_flux_divergence(
        self,
        shells: &SphericalShells<'_>,
        rhs: &mut [f64],
    ) -> Result<(), KernelError> {
        match self {
            Self::Compiled => spherical_flux_divergence(shells, rhs),
            Self::Reference => reference::spherical_flux_divergence(shells, rhs),
        }
    }

    fn minmod_slopes(self, values: &[f64], slopes: &mut [f64]) -> Result<(), KernelError> {
        match self {
            Self::Compiled => minmod_slopes(values, slopes),
            Self::Reference => reference::minmod_slopes(values, slopes),
        }
    }
}

/// Convert a grid size to `f64`.
///
/// The grid sizes here are a few thousand at most, so the conversion is exact.
fn as_f64(count: usize) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    let value = count as f64;
    value
}

/// Estimate the order of accuracy between each pair of successive grids.
///
/// The input errors must come from grids whose spacing is halved in turn.
fn observed_orders(errors: &[f64]) -> Vec<f64> {
    errors
        .windows(2)
        .map(|pair| (pair[0] / pair[1]).log2())
        .collect()
}

/// Require every observed order to lie within [`ORDER_TOLERANCE`] of `designed`.
///
/// `label` names the measurement so a failure identifies which kernel and
/// which implementation produced it.
fn assert_designed_order(label: &str, errors: &[f64], designed: f64) {
    assert!(
        errors.iter().all(|error| error.is_finite() && *error > 0.0),
        "{label}: errors must be finite and nonzero to estimate an order: {errors:?}"
    );
    let orders = observed_orders(errors);
    for (refinement, order) in orders.iter().enumerate() {
        assert!(
            (order - designed).abs() < ORDER_TOLERANCE,
            "{label}: refinement {refinement} converged at order {order}, not {designed} \
             (errors {errors:?})"
        );
    }
}

/// Innermost and outermost radius of the spherical grids, in arbitrary length
/// units. The inner boundary is away from the origin so that the exact
/// divergence, which carries a `1/r` term, stays bounded on the whole domain.
const INNER_RADIUS: f64 = 0.5;
/// Outer radius of the spherical grids.
const OUTER_RADIUS: f64 = 2.5;

/// Amplitude, wavenumber, and phase of the radial flux of each conserved
/// component.
///
/// The three components differ in all three parameters, so a kernel that reads
/// one component's flux while writing another's, or that collapses the stride,
/// produces a visibly wrong answer rather than a self-consistent one.
const FLUX_PARAMETERS: [(f64, f64, f64); CONSERVED_COUNT] =
    [(1.0, 2.0, 0.0), (-0.7, 1.3, 0.9), (0.4, 3.1, -1.7)];

/// Radial flux of `component` at radius `r`.
fn flux(component: usize, r: f64) -> f64 {
    let (amplitude, wavenumber, phase) = FLUX_PARAMETERS[component];
    amplitude * (wavenumber * r + phase).sin()
}

/// The exact spherical divergence of that flux.
///
/// For a purely radial field, `div F = r^-2 d(r^2 F)/dr = 2 F / r + dF/dr`.
fn exact_divergence(component: usize, r: f64) -> f64 {
    let (amplitude, wavenumber, phase) = FLUX_PARAMETERS[component];
    let angle = wavenumber * r + phase;
    2.0 * amplitude * angle.sin() / r + amplitude * wavenumber * angle.cos()
}

/// A spherical shell grid of `cells` uniformly spaced cells.
struct SphericalGrid {
    /// Interface areas, `4 pi r^2` at each of the `cells + 1` interfaces.
    interface_area: Vec<f64>,
    /// Exact shell volumes, `4 pi (r_out^3 - r_in^3) / 3`.
    cell_volume: Vec<f64>,
    /// Interface fluxes, component-fastest.
    interface_flux: Vec<f64>,
    /// Midpoint radius of each cell.
    cell_midpoint: Vec<f64>,
}

impl SphericalGrid {
    /// Build the grid, its exact geometry, and the exact flux at every
    /// interface.
    ///
    /// The areas and volumes are the exact measures of the shells rather than
    /// discretizations of them, so the only approximation the kernel makes is
    /// the one under test: representing a cell's mean divergence by its
    /// interface fluxes.
    fn new(cells: usize) -> Self {
        let spacing = (OUTER_RADIUS - INNER_RADIUS) / as_f64(cells);
        let radius = |face: usize| INNER_RADIUS + as_f64(face) * spacing;
        let mut interface_area = Vec::with_capacity(cells + 1);
        let mut interface_flux = Vec::with_capacity((cells + 1) * CONSERVED_COUNT);
        for face in 0..=cells {
            let r = radius(face);
            interface_area.push(4.0 * std::f64::consts::PI * r * r);
            for component in 0..CONSERVED_COUNT {
                interface_flux.push(flux(component, r));
            }
        }
        let mut cell_volume = Vec::with_capacity(cells);
        let mut cell_midpoint = Vec::with_capacity(cells);
        for cell in 0..cells {
            let (inner, outer) = (radius(cell), radius(cell + 1));
            cell_volume.push(4.0 * std::f64::consts::PI * (outer.powi(3) - inner.powi(3)) / 3.0);
            cell_midpoint.push(f64::midpoint(inner, outer));
        }
        Self {
            interface_area,
            cell_volume,
            interface_flux,
            cell_midpoint,
        }
    }

    /// Borrow the grid as kernel input, with a source term of `source`.
    fn shells<'a>(&'a self, source: &'a [f64]) -> SphericalShells<'a> {
        SphericalShells {
            interface_flux: &self.interface_flux,
            interface_area: &self.interface_area,
            cell_volume: &self.cell_volume,
            source,
        }
    }
}

/// The largest error the divergence kernel makes on a grid of `cells` cells.
///
/// The kernel returns `source - div F` averaged over each cell; with a zero
/// source the result is compared against `-div F` at the cell midpoint. The
/// error is normalized by the peak exact divergence so that the ratio between
/// grids is a ratio of relative errors.
fn divergence_error(implementation: Implementation, cells: usize) -> f64 {
    let grid = SphericalGrid::new(cells);
    let source = vec![0.0; cells * CONSERVED_COUNT];
    let mut rhs = vec![0.0; cells * CONSERVED_COUNT];
    implementation
        .spherical_flux_divergence(&grid.shells(&source), &mut rhs)
        .expect("a well-formed spherical grid is accepted");

    let mut peak = 0.0_f64;
    let mut error = 0.0_f64;
    for cell in 0..cells {
        let r = grid.cell_midpoint[cell];
        for component in 0..CONSERVED_COUNT {
            let expected = -exact_divergence(component, r);
            peak = peak.max(expected.abs());
            error = error.max((rhs[cell * CONSERVED_COUNT + component] - expected).abs());
        }
    }
    error / peak
}

/// The finite-volume divergence converges to the exact divergence at second
/// order.
///
/// A cell's interface fluxes give its *volume-averaged* divergence exactly, by
/// the divergence theorem. Reading that average as a value at the cell midpoint
/// is what costs accuracy, and it costs exactly two orders. This is the only
/// statement in the suite that anchors the kernel to something outside the
/// project: `2 F / r + dF/dr` is differentiated by hand, not computed by
/// Hydrax.
#[test]
fn the_spherical_divergence_converges_at_second_order() {
    for implementation in Implementation::BOTH {
        let errors = (0..REFINEMENTS)
            .map(|refinement| divergence_error(implementation, 40 << refinement))
            .collect::<Vec<_>>();
        assert_designed_order(&format!("{implementation:?} divergence"), &errors, 2.0);
    }
}

/// The divergence kernel conserves exactly, not merely to truncation error.
///
/// Summing the right-hand side over the domain, weighted by cell volume, must
/// leave only the flux through the two outer boundaries: every interior
/// interface appears once with each sign. This is the discrete form of the
/// divergence theorem, it holds on any grid however coarse, and unlike the
/// convergence above it is an identity rather than a limit -- so the bound is
/// round-off, scaled by the largest term the sum passes through rather than by
/// its much smaller total.
#[test]
fn the_spherical_divergence_telescopes_to_the_boundary_flux() {
    for implementation in Implementation::BOTH {
        for cells in [1_usize, 2, 7, 64, 641] {
            let grid = SphericalGrid::new(cells);
            let source = vec![0.0; cells * CONSERVED_COUNT];
            let mut rhs = vec![0.0; cells * CONSERVED_COUNT];
            implementation
                .spherical_flux_divergence(&grid.shells(&source), &mut rhs)
                .expect("a well-formed spherical grid is accepted");

            for component in 0..CONSERVED_COUNT {
                let integral: f64 = (0..cells)
                    .map(|cell| rhs[cell * CONSERVED_COUNT + component] * grid.cell_volume[cell])
                    .sum();
                let inner = grid.interface_area[0] * grid.interface_flux[component];
                let outer = grid.interface_area[cells]
                    * grid.interface_flux[cells * CONSERVED_COUNT + component];
                let expected = inner - outer;
                // The cancellation in the sum can be many orders deep, so the
                // bound is set by the largest quantity summed rather than by
                // the residue that survives.
                let scale = (0..cells)
                    .map(|cell| {
                        (rhs[cell * CONSERVED_COUNT + component] * grid.cell_volume[cell]).abs()
                    })
                    .fold(inner.abs().max(outer.abs()), f64::max);
                let residual = (integral - expected).abs() / scale;
                assert!(
                    residual < 1.0e-12,
                    "{implementation:?}: component {component} on {cells} cells left \
                     a relative residual of {residual}"
                );
            }
        }
    }
}

/// The smooth, strictly monotone, strictly convex field the limiter is measured
/// on.
///
/// Monotone and convex is the regime in which minmod is *supposed* to be
/// accurate: both one-sided differences share a sign, so the limiter returns
/// one of them rather than zero. At an extremum it correctly returns zero
/// instead, which the unit tests cover; measuring an order there would measure
/// the clipping rather than the accuracy.
fn limited_field(x: f64) -> f64 {
    (0.5 * x).exp()
}

/// The exact derivative of that field.
fn limited_field_derivative(x: f64) -> f64 {
    0.5 * (0.5 * x).exp()
}

/// The largest error the limiter makes in the interior of a grid of `cells`.
///
/// The kernel returns undivided differences, so the slope is recovered by
/// dividing by the spacing. The first and last slopes are zero by contract --
/// a real domain supplies ghost zones there -- so they are excluded.
fn minmod_slope_error(implementation: Implementation, cells: usize) -> f64 {
    let spacing = 1.0 / as_f64(cells);
    let values = (0..=cells)
        .map(|index| limited_field(as_f64(index) * spacing))
        .collect::<Vec<_>>();
    let mut slopes = vec![0.0; values.len()];
    implementation
        .minmod_slopes(&values, &mut slopes)
        .expect("a nonempty field is accepted");

    (1..values.len() - 1)
        .map(|index| {
            let x = as_f64(index) * spacing;
            (slopes[index] / spacing - limited_field_derivative(x)).abs()
        })
        .fold(0.0, f64::max)
}

/// The limited slope is a first-order approximation of the derivative.
///
/// On monotone convex data minmod always returns the backward difference, which
/// approximates the derivative to within `h f'' / 2`. First order is not a
/// shortfall to be fixed: it is the accuracy that makes a reconstruction
/// second-order in space, and second-order convergence here would mean the
/// limiter was not limiting.
#[test]
fn the_minmod_limiter_recovers_the_derivative_at_first_order() {
    for implementation in Implementation::BOTH {
        let errors = (0..REFINEMENTS)
            .map(|refinement| minmod_slope_error(implementation, 100 << refinement))
            .collect::<Vec<_>>();
        assert_designed_order(&format!("{implementation:?} minmod"), &errors, 1.0);
    }
}
