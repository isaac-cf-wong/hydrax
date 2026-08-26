//! The version and layout contract with the compiled kernel library.

use std::{
    ffi::CStr,
    fmt::{Display, Formatter},
    mem::{align_of, size_of},
};

use crate::{
    CONSERVED_COUNT,
    error::{KernelError, KernelStatus},
    ffi,
};

/// The ABI version this crate is written against.
///
/// This is the crate's own statement of the contract, written by hand. It is
/// deliberately not derived from the header: a bump that reached the C and not
/// the Rust, or the reverse, is exactly what the version smoke test is for.
pub const ABI_VERSION: AbiVersion = AbiVersion::new(1, 0, 0);

/// A kernel ABI version.
///
/// Each component is a byte, which is the width the encoded form gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AbiVersion {
    /// Incompatible-change component.
    pub major: u8,
    /// Additive-change component.
    pub minor: u8,
    /// Behaviour-preserving-fix component.
    pub patch: u8,
}

impl AbiVersion {
    /// Build a version from its components.
    #[must_use]
    pub const fn new(major: u8, minor: u8, patch: u8) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// Pack the version into the encoded form the ABI passes around.
    #[must_use]
    pub const fn encoded(self) -> u32 {
        ((self.major as u32) << 16) | ((self.minor as u32) << 8) | (self.patch as u32)
    }

    /// Unpack an encoded version, or return `None` if it sets a bit no
    /// component owns.
    ///
    /// The narrowing casts cannot lose information: each is applied to a value
    /// already masked to eight bits.
    #[must_use]
    #[allow(
        clippy::cast_possible_truncation,
        reason = "each component is masked to eight bits before the cast"
    )]
    pub const fn from_encoded(encoded: u32) -> Option<Self> {
        if encoded > 0x00FF_FFFF {
            return None;
        }
        Some(Self::new(
            ((encoded >> 16) & 0xFF) as u8,
            ((encoded >> 8) & 0xFF) as u8,
            (encoded & 0xFF) as u8,
        ))
    }

    /// Report whether a library at this version can serve a caller written
    /// against `caller`.
    ///
    /// The major components must agree, and the library's minor component must
    /// be at least the caller's: the library may have gained entry points the
    /// caller does not know about, but it may not be missing one the caller
    /// does. The patch component is not consulted.
    #[must_use]
    pub const fn serves(self, caller: Self) -> bool {
        self.major == caller.major && self.minor >= caller.minor
    }
}

impl Display for AbiVersion {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The memory layout of the ABI, as one side of the boundary sees it.
///
/// A version check cannot catch a descriptor that gained a field without
/// anyone remembering to bump the version; comparing the two sides' layouts
/// can. The widths are fixed at 64 bits so the record itself has one layout on
/// every target.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// Encoded ABI version.
    pub abi_version: u32,
    /// Conserved variables per zone.
    pub conserved_count: u32,
    /// Size of this record.
    pub size_of_layout: u64,
    /// Alignment of this record.
    pub align_of_layout: u64,
    /// Size of the Riemann-input descriptor.
    pub size_of_riemann_input: u64,
    /// Alignment of the Riemann-input descriptor.
    pub align_of_riemann_input: u64,
    /// Size of the shell-input descriptor.
    pub size_of_shell_input: u64,
    /// Alignment of the shell-input descriptor.
    pub align_of_shell_input: u64,
    /// Size of the floating-point type crossing the boundary.
    pub size_of_real: u64,
    /// Size of the index type crossing the boundary.
    pub size_of_index: u64,
}

/// Widen a size or alignment for the layout record.
///
/// `usize` is at most 64 bits on every target Rust supports, so this cast
/// cannot lose a value; the annotation is here because Clippy reasons about
/// the cast without knowing that.
#[allow(
    clippy::cast_possible_truncation,
    reason = "usize is at most 64 bits on every supported target"
)]
const fn widen(value: usize) -> u64 {
    value as u64
}

/// Narrow the conserved-variable count for the layout record.
///
/// The count is a small compile-time constant, which the assertion below
/// pins: a value that could not survive the cast fails the build rather than
/// the comparison.
#[allow(
    clippy::cast_possible_truncation,
    reason = "the conserved-variable count is a small compile-time constant"
)]
const fn conserved_count() -> u32 {
    assert!(
        CONSERVED_COUNT <= u32::MAX as usize,
        "the conserved-variable count must fit the layout record"
    );
    CONSERVED_COUNT as u32
}

impl Layout {
    /// The layout this crate's own declarations imply.
    ///
    /// Compare it against [`layout`] to detect a header that no longer
    /// describes the library it is linked against.
    #[must_use]
    pub const fn expected() -> Self {
        Self {
            abi_version: ABI_VERSION.encoded(),
            conserved_count: conserved_count(),
            size_of_layout: widen(size_of::<Self>()),
            align_of_layout: widen(align_of::<Self>()),
            size_of_riemann_input: widen(size_of::<ffi::RiemannInput>()),
            align_of_riemann_input: widen(align_of::<ffi::RiemannInput>()),
            size_of_shell_input: widen(size_of::<ffi::ShellInput>()),
            align_of_shell_input: widen(align_of::<ffi::ShellInput>()),
            size_of_real: widen(size_of::<f64>()),
            size_of_index: widen(size_of::<usize>()),
        }
    }
}

/// Return the ABI version the linked kernel library was compiled with.
///
/// # Panics
///
/// Panics if the library reports an encoded version that sets a bit no
/// component owns, which cannot happen for a library built from this header.
#[must_use]
pub fn library_version() -> AbiVersion {
    let encoded = ffi::hydrax_kernels_abi_version();
    AbiVersion::from_encoded(encoded)
        .unwrap_or_else(|| panic!("the kernel library reported malformed ABI version {encoded:#x}"))
}

/// Return the layout the linked kernel library reports.
///
/// # Panics
///
/// Panics if the library rejects a non-null destination, which cannot happen
/// for a library built from this header.
#[must_use]
pub fn layout() -> Layout {
    let mut layout = Layout::expected();
    // SAFETY: `hydrax_kernels_layout_query` writes every field of one
    // `hydrax_kernels_layout` through the pointer and reads nothing. The
    // pointer is to a live, uniquely borrowed local of the mirrored `#[repr(C)]`
    // type, so it is non-null, aligned, and large enough. The initial value is
    // overwritten in full; it is there only because the local must be
    // initialised before it is borrowed.
    let status = unsafe { ffi::hydrax_kernels_layout_query(&raw mut layout) };
    assert!(
        status == KernelStatus::Ok.code(),
        "the kernel library rejected a non-null layout destination with status {status}"
    );
    layout
}

/// Return the name of the compiled kernel backend.
///
/// # Panics
///
/// Panics if the library returns a name that is not valid UTF-8, which cannot
/// happen for a library built from this source tree.
#[must_use]
pub fn backend_name() -> &'static str {
    let name = ffi::hydrax_kernels_backend_name();
    assert!(
        !name.is_null(),
        "the kernel library reported no backend name"
    );
    // SAFETY: the ABI specifies a static, NUL-terminated string with static
    // storage duration, so the pointer is valid for reads up to its terminator
    // and for the whole program. It was just checked to be non-null.
    let name = unsafe { CStr::from_ptr(name) };
    name.to_str()
        .expect("the kernel library reported a backend name that is not UTF-8")
}

/// Return the library's description of a status code, or `None` if the library
/// does not define it.
#[must_use]
pub fn library_status_message(code: i32) -> Option<&'static str> {
    let message = ffi::hydrax_kernels_status_message(code);
    if message.is_null() {
        return None;
    }
    // SAFETY: the ABI specifies a static, NUL-terminated string with static
    // storage duration for every code it defines, and null for the rest, which
    // was just excluded.
    let message = unsafe { CStr::from_ptr(message) };
    message.to_str().ok()
}

/// Ask the library whether it can serve a caller at `caller`.
///
/// This is the library's own answer rather than [`AbiVersion::serves`] applied
/// to the reported version: the two are compared in the version smoke test, so
/// a library that changed the compatibility rule cannot pass unnoticed.
///
/// # Errors
///
/// Returns [`KernelError::Rejected`] with
/// [`KernelStatus::AbiMismatch`] if the library refuses.
pub fn library_accepts(caller: AbiVersion) -> Result<(), KernelError> {
    KernelError::from_status(ffi::hydrax_kernels_abi_check(caller.encoded()))
}

/// Check that the linked kernel library implements the ABI this crate expects.
///
/// The kernels are statically linked into the build that calls them, so today
/// this can only fail if the header and the sources disagree. It is the check
/// a dynamically loaded or vendor-supplied backend will need, and it costs one
/// call.
///
/// # Errors
///
/// Returns [`KernelError::Incompatible`] if the library's version cannot serve
/// this crate, and [`KernelError::Rejected`] if the library itself refuses.
pub fn ensure_compatible() -> Result<(), KernelError> {
    let found = library_version();
    if !found.serves(ABI_VERSION) {
        return Err(KernelError::Incompatible {
            expected: ABI_VERSION,
            found,
        });
    }
    library_accepts(ABI_VERSION)
}

#[cfg(test)]
mod tests {
    use super::AbiVersion;

    #[test]
    fn encoding_round_trips_over_every_component_boundary() {
        for major in [0_u8, 1, 127, 128, 255] {
            for minor in [0_u8, 1, 127, 128, 255] {
                for patch in [0_u8, 1, 127, 128, 255] {
                    let version = AbiVersion::new(major, minor, patch);
                    assert_eq!(AbiVersion::from_encoded(version.encoded()), Some(version));
                }
            }
        }
    }

    #[test]
    fn an_encoded_version_with_a_stray_high_bit_is_rejected() {
        assert_eq!(
            AbiVersion::from_encoded(0x00FF_FFFF),
            Some(AbiVersion::new(255, 255, 255))
        );
        for encoded in [0x0100_0000, 0x0100_0001, u32::MAX] {
            assert_eq!(AbiVersion::from_encoded(encoded), None);
        }
    }

    #[test]
    fn compatibility_needs_the_same_major_and_at_least_the_caller_minor() {
        let library = AbiVersion::new(1, 2, 3);
        for (caller, expected) in [
            (AbiVersion::new(1, 2, 3), true),
            // The patch component is not part of the decision.
            (AbiVersion::new(1, 2, 0), true),
            (AbiVersion::new(1, 2, 255), true),
            // A caller wanting less of the interface than the library offers.
            (AbiVersion::new(1, 0, 0), true),
            // A caller wanting an entry point the library may not have.
            (AbiVersion::new(1, 3, 0), false),
            // A different major is incompatible in either direction.
            (AbiVersion::new(0, 2, 3), false),
            (AbiVersion::new(2, 2, 3), false),
            (AbiVersion::new(0, 255, 255), false),
        ] {
            assert_eq!(
                library.serves(caller),
                expected,
                "library {library} serving caller {caller}"
            );
        }
    }
}
