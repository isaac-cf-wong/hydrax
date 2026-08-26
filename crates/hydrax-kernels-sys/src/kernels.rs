//! Safe wrappers around the compiled kernels.
//!
//! Every wrapper checks the buffer lengths the ABI leaves to the caller before
//! it forms a pointer. That check is what makes the `unsafe` blocks here sound:
//! the C reads exactly the number of elements the domain size implies, and the
//! lengths are proved to match first.

use crate::{CONSERVED_COUNT, error::KernelError, ffi};

/// Reconstructed interface states and their signal speeds.
///
/// The state and flux buffers hold `interface_count * CONSERVED_COUNT` values,
/// component-fastest; the speed buffers hold one value per interface. The
/// interface count is taken from `left_speed`, and every other buffer is
/// checked against it.
#[derive(Debug, Clone, Copy)]
pub struct RiemannStates<'a> {
    /// Left conserved state.
    pub left_state: &'a [f64],
    /// Right conserved state.
    pub right_state: &'a [f64],
    /// Physical flux of the left state.
    pub left_flux: &'a [f64],
    /// Physical flux of the right state.
    pub right_flux: &'a [f64],
    /// Leftmost signal speed per interface.
    pub left_speed: &'a [f64],
    /// Rightmost signal speed per interface.
    pub right_speed: &'a [f64],
}

impl RiemannStates<'_> {
    /// Number of interfaces the input describes.
    #[must_use]
    pub const fn interface_count(&self) -> usize {
        self.left_speed.len()
    }

    /// Check every buffer against the interface count.
    ///
    /// # Errors
    ///
    /// Returns [`KernelError::LengthMismatch`] naming the first buffer whose
    /// length the interface count does not explain.
    pub fn validate_lengths(&self, flux: &[f64]) -> Result<(), KernelError> {
        let interfaces = self.interface_count();
        let Some(values) = interfaces.checked_mul(CONSERVED_COUNT) else {
            return Err(KernelError::Rejected(crate::KernelStatus::SizeOverflow));
        };
        for (buffer, actual) in [
            ("left_state", self.left_state.len()),
            ("right_state", self.right_state.len()),
            ("left_flux", self.left_flux.len()),
            ("right_flux", self.right_flux.len()),
            ("flux", flux.len()),
        ] {
            check_length(buffer, values, actual)?;
        }
        check_length("right_speed", interfaces, self.right_speed.len())
    }
}

/// Interface fluxes and shell geometry of a spherical grid.
///
/// The cell count is taken from `cell_volume`, and every other buffer is
/// checked against it.
#[derive(Debug, Clone, Copy)]
pub struct SphericalShells<'a> {
    /// Interface flux, one entry per interface and component.
    pub interface_flux: &'a [f64],
    /// Interface area, one entry per interface.
    pub interface_area: &'a [f64],
    /// Cell volume, one entry per cell.
    pub cell_volume: &'a [f64],
    /// Source term, one entry per cell and component.
    pub source: &'a [f64],
}

impl SphericalShells<'_> {
    /// Number of cells the input describes.
    #[must_use]
    pub const fn cell_count(&self) -> usize {
        self.cell_volume.len()
    }

    /// Check every buffer against the cell count.
    ///
    /// # Errors
    ///
    /// Returns [`KernelError::LengthMismatch`] naming the first buffer whose
    /// length the cell count does not explain.
    pub fn validate_lengths(&self, rhs: &[f64]) -> Result<(), KernelError> {
        let cells = self.cell_count();
        let interfaces = cells
            .checked_add(1)
            .and_then(|interfaces| interfaces.checked_mul(CONSERVED_COUNT));
        let (Some(interface_values), Some(cell_values)) =
            (interfaces, cells.checked_mul(CONSERVED_COUNT))
        else {
            return Err(KernelError::Rejected(crate::KernelStatus::SizeOverflow));
        };
        check_length(
            "interface_flux",
            interface_values,
            self.interface_flux.len(),
        )?;
        check_length("interface_area", cells + 1, self.interface_area.len())?;
        check_length("source", cell_values, self.source.len())?;
        check_length("rhs", cell_values, rhs.len())
    }
}

/// Reject a buffer whose length the domain size does not explain.
pub(crate) fn check_length(
    buffer: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), KernelError> {
    if expected == actual {
        Ok(())
    } else {
        Err(KernelError::LengthMismatch {
            buffer,
            expected,
            actual,
        })
    }
}

/// Apply the minmod limiter to the undivided differences of a scalar field.
///
/// Slope `i` is `minmod(values[i] - values[i - 1], values[i + 1] - values[i])`,
/// where `minmod` returns the argument of smaller magnitude when both share a
/// sign and zero otherwise. The first and last slopes are zero; a real domain
/// supplies ghost zones there.
///
/// # Errors
///
/// Returns [`KernelError::LengthMismatch`] if `slopes` is not as long as
/// `values`, and [`KernelError::Rejected`] with
/// [`KernelStatus::EmptyDomain`](crate::KernelStatus::EmptyDomain) for an empty
/// field.
pub fn minmod_slopes(values: &[f64], slopes: &mut [f64]) -> Result<(), KernelError> {
    check_length("slopes", values.len(), slopes.len())?;
    if values.is_empty() {
        // The pointers of an empty slice are dangling, so the call is not made
        // at all; the status is the one the ABI specifies for this input.
        return Err(KernelError::Rejected(crate::KernelStatus::EmptyDomain));
    }
    // SAFETY: both pointers come from live slices of `values.len()` elements,
    // which is the count passed to the kernel and was just checked against
    // `slopes`. The kernel reads that many from `values` and writes that many
    // to `slopes`; the slices are distinct because one is uniquely borrowed.
    let code = unsafe {
        ffi::hydrax_kernels_minmod_slopes(values.as_ptr(), values.len(), slopes.as_mut_ptr())
    };
    KernelError::from_status(code)
}

/// Evaluate the HLL numerical flux at every interface.
///
/// # Errors
///
/// Returns [`KernelError::LengthMismatch`] if a buffer length does not match
/// the interface count, and [`KernelError::Rejected`] for an empty domain or a
/// signal-speed pair that is not a finite, correctly ordered interval.
pub fn hll_flux(states: &RiemannStates<'_>, flux: &mut [f64]) -> Result<(), KernelError> {
    states.validate_lengths(flux)?;
    if states.interface_count() == 0 {
        return Err(KernelError::Rejected(crate::KernelStatus::EmptyDomain));
    }
    let input = ffi::RiemannInput {
        left_state: states.left_state.as_ptr(),
        right_state: states.right_state.as_ptr(),
        left_flux: states.left_flux.as_ptr(),
        right_flux: states.right_flux.as_ptr(),
        left_speed: states.left_speed.as_ptr(),
        right_speed: states.right_speed.as_ptr(),
        interface_count: states.interface_count(),
    };
    // SAFETY: every pointer in `input` comes from a live slice whose length
    // `validate_lengths` proved equal to what `interface_count` makes the
    // kernel read, and the descriptor itself is a live local of the mirrored
    // `#[repr(C)]` type. `flux` is uniquely borrowed and holds the number of
    // values the kernel writes, so it overlaps none of the inputs.
    let code = unsafe { ffi::hydrax_kernels_hll_flux(&raw const input, flux.as_mut_ptr()) };
    KernelError::from_status(code)
}

/// Assemble the semi-discrete right-hand side of a spherical finite-volume
/// update.
///
/// # Errors
///
/// Returns [`KernelError::LengthMismatch`] if a buffer length does not match
/// the cell count, and [`KernelError::Rejected`] for an empty domain or a cell
/// volume or interface area that is not a usable geometric measure.
pub fn spherical_flux_divergence(
    shells: &SphericalShells<'_>,
    rhs: &mut [f64],
) -> Result<(), KernelError> {
    shells.validate_lengths(rhs)?;
    if shells.cell_count() == 0 {
        return Err(KernelError::Rejected(crate::KernelStatus::EmptyDomain));
    }
    let input = ffi::ShellInput {
        interface_flux: shells.interface_flux.as_ptr(),
        interface_area: shells.interface_area.as_ptr(),
        cell_volume: shells.cell_volume.as_ptr(),
        source: shells.source.as_ptr(),
        cell_count: shells.cell_count(),
    };
    // SAFETY: as for `hll_flux`. Every pointer in `input` comes from a live
    // slice whose length `validate_lengths` proved equal to what `cell_count`
    // makes the kernel read, and `rhs` is uniquely borrowed and exactly as long
    // as the kernel writes.
    let code = unsafe {
        ffi::hydrax_kernels_spherical_flux_divergence(&raw const input, rhs.as_mut_ptr())
    };
    KernelError::from_status(code)
}
