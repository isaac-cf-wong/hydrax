//! Raw declarations of the C entry points.
//!
//! These mirror `kernels/include/hydrax_kernels.h` one for one. Nothing here
//! validates anything; the safe wrappers in [`crate::kernels`] and
//! [`crate::abi`] are what turn a raw call into a checked one.

use std::os::raw::c_char;

/// Status code returned by every kernel entry point.
///
/// The C side returns `int32_t` rather than the enumeration type, because the
/// underlying type of a C enumeration is implementation-defined and a backend
/// built by another compiler must not be able to disagree about its width.
pub(crate) type Status = i32;

/// Reconstructed interface states and their signal speeds.
///
/// Mirrors `hydrax_kernels_riemann_input`. The pointers are unchecked and
/// untracked; a value of this type is built inside a safe wrapper from slices
/// that outlive the call.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct RiemannInput {
    /// Left conserved state.
    pub(crate) left_state: *const f64,
    /// Right conserved state.
    pub(crate) right_state: *const f64,
    /// Physical flux of the left state.
    pub(crate) left_flux: *const f64,
    /// Physical flux of the right state.
    pub(crate) right_flux: *const f64,
    /// Leftmost signal speed per interface.
    pub(crate) left_speed: *const f64,
    /// Rightmost signal speed per interface.
    pub(crate) right_speed: *const f64,
    /// Number of interfaces.
    pub(crate) interface_count: usize,
}

/// Interface fluxes and shell geometry of a spherical grid.
///
/// Mirrors `hydrax_kernels_shell_input`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct ShellInput {
    /// Interface flux.
    pub(crate) interface_flux: *const f64,
    /// Interface area.
    pub(crate) interface_area: *const f64,
    /// Cell volume.
    pub(crate) cell_volume: *const f64,
    /// Source term.
    pub(crate) source: *const f64,
    /// Number of cells.
    pub(crate) cell_count: usize,
}

unsafe extern "C" {
    /// Return the encoded ABI version the library was compiled with.
    pub(crate) safe fn hydrax_kernels_abi_version() -> u32;

    /// Report whether a caller compiled against `caller_version` may use the
    /// library.
    pub(crate) safe fn hydrax_kernels_abi_check(caller_version: u32) -> Status;

    /// Write the library's view of the ABI layout into `layout`.
    pub(crate) fn hydrax_kernels_layout_query(layout: *mut crate::Layout) -> Status;

    /// Return the name of the compiled backend.
    pub(crate) safe fn hydrax_kernels_backend_name() -> *const c_char;

    /// Return a description of a status code, or null for an unknown code.
    pub(crate) safe fn hydrax_kernels_status_message(status: Status) -> *const c_char;

    /// Apply the minmod limiter to the undivided differences of a scalar field.
    pub(crate) fn hydrax_kernels_minmod_slopes(
        values: *const f64,
        cell_count: usize,
        slopes: *mut f64,
    ) -> Status;

    /// Evaluate the HLL numerical flux at every interface.
    pub(crate) fn hydrax_kernels_hll_flux(input: *const RiemannInput, flux: *mut f64) -> Status;

    /// Assemble the semi-discrete right-hand side of a spherical update.
    pub(crate) fn hydrax_kernels_spherical_flux_divergence(
        input: *const ShellInput,
        rhs: *mut f64,
    ) -> Status;
}
