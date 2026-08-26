//! Versioned C ABI bindings to the Hydrax numerical kernels.
//!
//! Hydrax solves relativistic hydrodynamics in Rust and hands the innermost
//! loops to compiled kernels. This crate is the whole of that boundary: the
//! declarations of the C entry points, the safe wrappers that check what the
//! ABI leaves to the caller, and the version and layout checks that decide
//! whether a compiled backend may be used at all.
//!
//! It is also the only crate in the workspace where `unsafe` is permitted.
//! Every other crate sets `unsafe_code = "forbid"`, which is only a meaningful
//! statement about the project because the exception is confined here and kept
//! small.
//!
//! # The contract
//!
//! [`kernels/include/hydrax_kernels.h`][header] is the single source of truth
//! for the interface. A backend -- the portable C11 reference compiled today,
//! a CUDA or HIP implementation later -- exports those symbols with C linkage
//! and is interchangeable with any other. The ABI carries [`ABI_VERSION`],
//! versioned independently of the crate, so a kernel signature cannot change
//! behind an unchanged crate version, and [`Layout`] lets the two sides compare
//! the sizes and alignments a version check cannot see.
//!
//! [header]: https://github.com/isaac-cf-wong/hydrax/blob/main/kernels/include/hydrax_kernels.h
//!
//! Three independent statements of that contract are kept, and the version
//! smoke test asserts they agree:
//!
//! 1. the constants written by hand in this crate, [`ABI_VERSION`] and
//!    [`CONSERVED_COUNT`];
//! 2. the macros written by hand in the header, mirrored into [`header`] by the
//!    build script;
//! 3. the values the compiled library reports at run time, through
//!    [`library_version`] and [`layout`].
//!
//! No two of those could establish the contract on their own: a value derived
//! from another agrees with it by construction.
//!
//! # Differential testing
//!
//! Every kernel has a slow safe-Rust twin in [`mod@reference`], with the same
//! signature and the same errors. The compiled kernels are checked against
//! those twins over inputs that reach each branch, which is how a
//! GPU implementation of the same ABI will be checked too.
//!
//! # Example
//!
//! ```
//! use hydrax_kernels_sys::{SphericalShells, ensure_compatible, spherical_flux_divergence};
//!
//! ensure_compatible()?;
//!
//! // Two shells, three conserved variables, a flux that is uniform in radius.
//! let shells = SphericalShells {
//!     interface_flux: &[1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
//!     interface_area: &[1.0, 2.0, 3.0],
//!     cell_volume: &[0.5, 0.5],
//!     source: &[0.0; 6],
//! };
//! let mut rhs = [0.0; 6];
//! spherical_flux_divergence(&shells, &mut rhs)?;
//!
//! // A uniform flux through a growing area is still a divergence.
//! assert!((rhs[0] - -2.0).abs() < 1e-15);
//! assert!((rhs[3] - -2.0).abs() < 1e-15);
//! # Ok::<(), hydrax_kernels_sys::KernelError>(())
//! ```

mod abi;
mod error;
mod ffi;
mod kernels;
pub mod reference;

pub use abi::{
    ABI_VERSION, AbiVersion, Layout, backend_name, ensure_compatible, layout, library_accepts,
    library_status_message, library_version,
};
pub use error::{KernelError, KernelStatus};
pub use kernels::{
    RiemannStates, SphericalShells, hll_flux, minmod_slopes, spherical_flux_divergence,
};

/// Number of conserved variables carried per zone.
///
/// Spherical relativistic hydrodynamics in the Valencia formulation evolves the
/// conserved rest-mass density, the radial momentum density, and the rescaled
/// energy density. The count fixes the stride of every state and flux buffer
/// crossing the ABI, which is why it is part of the contract rather than a
/// detail of a kernel.
pub const CONSERVED_COUNT: usize = 3;

/// What the C header declared when this crate was built.
///
/// The build script reads these out of `kernels/include/hydrax_kernels.h`. They
/// are the header's own statement of the contract, independent of the constants
/// written by hand in this crate and of the values the compiled library reports
/// at run time.
pub mod header {
    include!(concat!(env!("OUT_DIR"), "/header_constants.rs"));

    /// The ABI version the header declared.
    #[must_use]
    pub const fn abi_version() -> crate::AbiVersion {
        crate::AbiVersion::new(ABI_VERSION_MAJOR, ABI_VERSION_MINOR, ABI_VERSION_PATCH)
    }
}
