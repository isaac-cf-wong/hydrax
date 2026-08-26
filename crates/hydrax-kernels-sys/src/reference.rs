//! Slow safe-Rust references for every kernel.
//!
//! These exist to be disagreed with. A kernel that is fast, written in another
//! language, and eventually running on a GPU cannot be trusted on inspection;
//! it can be trusted when it reproduces an implementation short enough to read
//! in one sitting, over inputs chosen to reach every branch. The differential
//! test in `tests/differential.rs` is what makes that comparison, and these are
//! the implementations it compares against.
//!
//! They are deliberately unhurried: each builds its whole result in a fresh
//! allocation before copying it out, and none of them is written for speed. The
//! one thing they do follow is the order of the floating-point operations in
//! the C, so that a disagreement means a difference in the algorithm rather
//! than in the rounding.
//!
//! The buffer-length contract is the Rust wrapper's rather than the kernel's,
//! so the length checks are shared with the wrappers instead of written twice.
//! Everything a kernel itself decides -- the emptiness, geometry, and
//! signal-speed rejections, and every arithmetic result -- is implemented here
//! independently.

use crate::{
    CONSERVED_COUNT, KernelError, KernelStatus, RiemannStates, SphericalShells,
    kernels::check_length,
};

/// Return the argument of smaller magnitude when both share a sign, else zero.
///
/// A NaN argument fails both sign tests and so limits the slope to zero, which
/// is the conservative answer for a limiter.
fn minmod(left: f64, right: f64) -> f64 {
    let sign = if left > 0.0 && right > 0.0 {
        1.0
    } else if left < 0.0 && right < 0.0 {
        -1.0
    } else {
        return 0.0;
    };
    sign * left.abs().min(right.abs())
}

/// Apply the minmod limiter to the undivided differences of a scalar field.
///
/// See [`crate::minmod_slopes`] for the contract.
///
/// # Errors
///
/// Returns the same errors as [`crate::minmod_slopes`].
pub fn minmod_slopes(values: &[f64], slopes: &mut [f64]) -> Result<(), KernelError> {
    check_length("slopes", values.len(), slopes.len())?;
    if values.is_empty() {
        return Err(KernelError::Rejected(KernelStatus::EmptyDomain));
    }
    let last = values.len() - 1;
    let limited = (0..values.len())
        .map(|cell| {
            if cell == 0 || cell == last {
                0.0
            } else {
                minmod(
                    values[cell] - values[cell - 1],
                    values[cell + 1] - values[cell],
                )
            }
        })
        .collect::<Vec<_>>();
    slopes.copy_from_slice(&limited);
    Ok(())
}

/// Evaluate the HLL numerical flux at every interface.
///
/// See [`crate::hll_flux`] for the contract.
///
/// # Errors
///
/// Returns the same errors as [`crate::hll_flux`].
pub fn hll_flux(states: &RiemannStates<'_>, flux: &mut [f64]) -> Result<(), KernelError> {
    states.validate_lengths(flux)?;
    let interfaces = states.interface_count();
    if interfaces == 0 {
        return Err(KernelError::Rejected(KernelStatus::EmptyDomain));
    }
    let speeds = states
        .left_speed
        .iter()
        .copied()
        .zip(states.right_speed.iter().copied())
        .collect::<Vec<_>>();
    if speeds
        .iter()
        .any(|(slowest, fastest)| !slowest.is_finite() || !fastest.is_finite() || slowest > fastest)
    {
        return Err(KernelError::Rejected(KernelStatus::DegenerateWaveSpeeds));
    }

    let mut computed = Vec::with_capacity(interfaces * CONSERVED_COUNT);
    for (face, (slowest, fastest)) in speeds.into_iter().enumerate() {
        let base = face * CONSERVED_COUNT;
        for component in 0..CONSERVED_COUNT {
            let index = base + component;
            let left_flux = states.left_flux[index];
            let right_flux = states.right_flux[index];
            let value = if slowest >= 0.0 {
                left_flux
            } else if fastest <= 0.0 {
                right_flux
            } else {
                let jump = states.right_state[index] - states.left_state[index];
                (fastest * left_flux - slowest * right_flux + slowest * fastest * jump)
                    / (fastest - slowest)
            };
            computed.push(value);
        }
    }
    flux.copy_from_slice(&computed);
    Ok(())
}

/// Assemble the semi-discrete right-hand side of a spherical finite-volume
/// update.
///
/// See [`crate::spherical_flux_divergence`] for the contract.
///
/// # Errors
///
/// Returns the same errors as [`crate::spherical_flux_divergence`].
pub fn spherical_flux_divergence(
    shells: &SphericalShells<'_>,
    rhs: &mut [f64],
) -> Result<(), KernelError> {
    shells.validate_lengths(rhs)?;
    let cells = shells.cell_count();
    if cells == 0 {
        return Err(KernelError::Rejected(KernelStatus::EmptyDomain));
    }
    let geometry_is_usable = shells
        .interface_area
        .iter()
        .all(|area| area.is_finite() && *area >= 0.0)
        && shells
            .cell_volume
            .iter()
            .all(|volume| volume.is_finite() && *volume > 0.0);
    if !geometry_is_usable {
        return Err(KernelError::Rejected(KernelStatus::InvalidGeometry));
    }

    let mut computed = Vec::with_capacity(cells * CONSERVED_COUNT);
    for cell in 0..cells {
        let inner_area = shells.interface_area[cell];
        let outer_area = shells.interface_area[cell + 1];
        let volume = shells.cell_volume[cell];
        for component in 0..CONSERVED_COUNT {
            let inner = cell * CONSERVED_COUNT + component;
            let outer = (cell + 1) * CONSERVED_COUNT + component;
            let inner_flux = shells.interface_flux[inner];
            let outer_flux = shells.interface_flux[outer];
            computed.push(
                shells.source[inner] - (outer_area * outer_flux - inner_area * inner_flux) / volume,
            );
        }
    }
    rhs.copy_from_slice(&computed);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::minmod;

    #[test]
    fn minmod_takes_the_smaller_magnitude_of_two_like_signed_slopes() {
        for (left, right, expected) in [
            (1.0, 2.0, 1.0),
            (2.0, 1.0, 1.0),
            (-1.0, -2.0, -1.0),
            (-2.0, -1.0, -1.0),
            (3.0, 3.0, 3.0),
        ] {
            assert!(
                (minmod(left, right) - expected).abs() < f64::EPSILON,
                "minmod({left}, {right})"
            );
        }
    }

    // The limiter's answer at an extremum is exactly zero, not nearly zero:
    // a slope that merely rounded to zero would still reconstruct a
    // discontinuity, so the comparison is deliberately exact.
    #[allow(clippy::float_cmp, reason = "the limiter must return exactly zero")]
    #[test]
    fn minmod_flattens_an_extremum_and_anything_involving_nan() {
        for (left, right) in [
            (1.0, -1.0),
            (-1.0, 1.0),
            (0.0, 1.0),
            (1.0, 0.0),
            (0.0, -0.0),
            (f64::NAN, 1.0),
            (1.0, f64::NAN),
            (f64::NAN, f64::NAN),
        ] {
            assert_eq!(minmod(left, right), 0.0, "minmod({left}, {right})");
        }
    }
}
