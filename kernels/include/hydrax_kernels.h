/*
 * Hydrax GPU kernel ABI.
 *
 * This header is the whole contract between the Rust workspace and the
 * compiled kernels. Everything the Rust side needs to know -- the version, the
 * status codes, the memory layout of the launch descriptors, and the entry
 * points -- is declared here and nowhere else, so a kernel backend can be
 * swapped for a GPU implementation without touching a line of Rust.
 *
 * The declarations are C11 and deliberately free of any C++ or CUDA
 * construct. A backend may be written in C++, CUDA, or HIP; it exports these
 * symbols with C linkage and stays interchangeable with every other backend.
 */

#ifndef HYDRAX_KERNELS_H
#define HYDRAX_KERNELS_H

#include <stddef.h>
#include <stdint.h>

#if defined(__cplusplus)
extern "C" {
#endif

/*
 * ---------------------------------------------------------------------------
 * Version
 * ---------------------------------------------------------------------------
 *
 * The ABI carries its own version, independent of the Hydrax crate version: a
 * crate release that leaves the kernel interface alone must not force every
 * backend to be rebuilt, and a change to a kernel signature must not be able
 * to hide behind an unchanged crate version.
 *
 * Bump the major component for any change that breaks a caller compiled
 * against an earlier header -- a removed entry point, a changed signature, a
 * reordered or resized descriptor field, a reused status code. Bump the minor
 * component for a purely additive change, such as a new entry point or a new
 * status code. Bump the patch component for a behaviour-preserving fix inside
 * an existing kernel.
 *
 * Each component occupies eight bits of the encoded version, so each is
 * limited to 0..=255.
 */

#define HYDRAX_KERNELS_ABI_VERSION_MAJOR 1
#define HYDRAX_KERNELS_ABI_VERSION_MINOR 0
#define HYDRAX_KERNELS_ABI_VERSION_PATCH 0

/** Pack a major/minor/patch triple into the encoded ABI version. */
#define HYDRAX_KERNELS_ABI_VERSION_ENCODE(major, minor, patch)                 \
    ((((uint32_t)(major)) << 16) | (((uint32_t)(minor)) << 8) |                \
     ((uint32_t)(patch)))

/** The ABI version this header declares. */
#define HYDRAX_KERNELS_ABI_VERSION                                             \
    HYDRAX_KERNELS_ABI_VERSION_ENCODE(HYDRAX_KERNELS_ABI_VERSION_MAJOR,        \
                                      HYDRAX_KERNELS_ABI_VERSION_MINOR,        \
                                      HYDRAX_KERNELS_ABI_VERSION_PATCH)

/** Extract the major component of an encoded ABI version. */
#define HYDRAX_KERNELS_ABI_MAJOR_OF(version)                                   \
    ((uint32_t)((((uint32_t)(version)) >> 16) & 0xFFu))

/** Extract the minor component of an encoded ABI version. */
#define HYDRAX_KERNELS_ABI_MINOR_OF(version)                                   \
    ((uint32_t)((((uint32_t)(version)) >> 8) & 0xFFu))

/** Extract the patch component of an encoded ABI version. */
#define HYDRAX_KERNELS_ABI_PATCH_OF(version)                                   \
    ((uint32_t)(((uint32_t)(version)) & 0xFFu))

/*
 * ---------------------------------------------------------------------------
 * Conventions
 * ---------------------------------------------------------------------------
 *
 * Every floating-point quantity crossing this boundary is a `double`. The
 * relativistic hydrodynamics the kernels serve is solved in double precision
 * throughout, and a mixed-precision boundary would hide a conversion inside
 * the interface rather than in the caller that chose it.
 *
 * A cell-centred or interface-centred state is stored component-fastest:
 * component `c` of zone `i` lives at index `i * HYDRAX_KERNELS_CONSERVED_COUNT
 * + c`. The kernels never allocate; every buffer is owned by the caller.
 */

/**
 * Number of conserved variables carried per zone.
 *
 * Spherical relativistic hydrodynamics in the Valencia formulation evolves the
 * conserved rest-mass density, the radial momentum density, and the rescaled
 * energy density. The count is part of the ABI because it fixes the stride of
 * every state and flux buffer.
 */
#define HYDRAX_KERNELS_CONSERVED_COUNT 3

/*
 * ---------------------------------------------------------------------------
 * Status codes
 * ---------------------------------------------------------------------------
 *
 * The return type is a fixed-width integer rather than the enumeration type
 * itself: the underlying type of a C enumeration is implementation-defined,
 * and a backend built by a different compiler than the caller must not be able
 * to disagree about the width of a return value.
 *
 * A code, once assigned, keeps its meaning forever. Retiring one is a
 * major-version change.
 */

/** Status returned by every kernel entry point. */
typedef int32_t hydrax_kernels_status;

/** The status codes a kernel may return. */
enum hydrax_kernels_status_code {
    /** The call succeeded. */
    HYDRAX_KERNELS_OK = 0,
    /** A required buffer pointer was null. */
    HYDRAX_KERNELS_ERR_NULL_POINTER = 1,
    /** The domain held no zones. */
    HYDRAX_KERNELS_ERR_EMPTY_DOMAIN = 2,
    /** The caller and the library disagree about the ABI. */
    HYDRAX_KERNELS_ERR_ABI_MISMATCH = 3,
    /** A cell volume or interface area was not a usable geometric measure. */
    HYDRAX_KERNELS_ERR_INVALID_GEOMETRY = 4,
    /** A signal-speed pair was not a finite, correctly ordered interval. */
    HYDRAX_KERNELS_ERR_DEGENERATE_WAVE_SPEEDS = 5,
    /** The requested domain size cannot be addressed on this target. */
    HYDRAX_KERNELS_ERR_SIZE_OVERFLOW = 6
};

/*
 * ---------------------------------------------------------------------------
 * Introspection
 * ---------------------------------------------------------------------------
 */

/**
 * Memory layout of the ABI, as the compiled library sees it.
 *
 * A caller compares these against its own view to detect a header that no
 * longer describes the library it is linked against -- a mismatch a version
 * check alone cannot catch, because a struct can change size without anyone
 * remembering to bump the version.
 *
 * The size and alignment fields are `uint64_t` rather than `size_t` so that
 * the layout record itself has one layout on every target.
 */
typedef struct hydrax_kernels_layout {
    /** Encoded ABI version the library was compiled with. */
    uint32_t abi_version;
    /** Conserved variables per zone. */
    uint32_t conserved_count;
    /** `sizeof(hydrax_kernels_layout)`. */
    uint64_t size_of_layout;
    /** `_Alignof(hydrax_kernels_layout)`. */
    uint64_t align_of_layout;
    /** `sizeof(hydrax_kernels_riemann_input)`. */
    uint64_t size_of_riemann_input;
    /** `_Alignof(hydrax_kernels_riemann_input)`. */
    uint64_t align_of_riemann_input;
    /** `sizeof(hydrax_kernels_shell_input)`. */
    uint64_t size_of_shell_input;
    /** `_Alignof(hydrax_kernels_shell_input)`. */
    uint64_t align_of_shell_input;
    /** `sizeof(double)`. */
    uint64_t size_of_real;
    /** `sizeof(size_t)`. */
    uint64_t size_of_index;
} hydrax_kernels_layout;

/** Return the encoded ABI version the library was compiled with. */
uint32_t hydrax_kernels_abi_version(void);

/**
 * Report whether a caller compiled against `caller_version` may use this
 * library.
 *
 * The major components must be equal, and the library's minor component must
 * be at least the caller's: a library may have gained entry points the caller
 * does not know about, but it may not be missing one the caller does. The
 * patch component is not consulted.
 *
 * Returns `HYDRAX_KERNELS_OK` or `HYDRAX_KERNELS_ERR_ABI_MISMATCH`.
 */
hydrax_kernels_status hydrax_kernels_abi_check(uint32_t caller_version);

/**
 * Write the library's view of the ABI layout into `layout`.
 *
 * Returns `HYDRAX_KERNELS_ERR_NULL_POINTER` if `layout` is null.
 */
hydrax_kernels_status hydrax_kernels_layout_query(hydrax_kernels_layout *layout);

/**
 * Return the name of the compiled backend, as a static NUL-terminated string.
 *
 * The name identifies the implementation behind the ABI -- currently the
 * portable reference backend -- so a run's provenance can record which set of
 * kernels produced its numbers.
 */
const char *hydrax_kernels_backend_name(void);

/**
 * Return a static description of a status code, or null if the code is not one
 * this library defines.
 *
 * The null return is what lets a caller check that its own table of status
 * codes still matches the library's.
 */
const char *hydrax_kernels_status_message(hydrax_kernels_status status);

/*
 * ---------------------------------------------------------------------------
 * Kernels
 * ---------------------------------------------------------------------------
 *
 * Every kernel validates its whole input before writing anything, so a
 * rejected call leaves the output buffer untouched. That is what makes a
 * failure recoverable: the caller can correct the input and retry without
 * having to know how far the kernel got.
 *
 * Validation covers the geometry and the wave speeds -- the quantities whose
 * meaning the kernel depends on -- and not the state and flux payloads, whose
 * admissible range is the caller's physics rather than the kernel's. A
 * non-finite payload therefore propagates to the output rather than being
 * rejected.
 */

/**
 * Apply the minmod limiter to the undivided differences of a cell-centred
 * scalar field.
 *
 * `values` and `slopes` hold `cell_count` entries each. Slope `i` is
 * `minmod(values[i] - values[i - 1], values[i + 1] - values[i])`, where
 * `minmod` returns the argument of smaller magnitude when both share a sign
 * and zero otherwise. The first and last slopes are zero: a one-sided
 * difference at the edge of the array is not a limited slope, and a real
 * domain supplies ghost zones there.
 *
 * The differences are undivided. On a non-uniform grid the caller divides by
 * the cell width; keeping the division out of the kernel keeps one grid
 * convention out of the ABI.
 *
 * Returns `HYDRAX_KERNELS_ERR_NULL_POINTER` for a null buffer and
 * `HYDRAX_KERNELS_ERR_EMPTY_DOMAIN` for `cell_count == 0`.
 */
hydrax_kernels_status hydrax_kernels_minmod_slopes(const double *values,
                                                   size_t cell_count,
                                                   double *slopes);

/** Reconstructed interface states and their signal speeds. */
typedef struct hydrax_kernels_riemann_input {
    /** Left conserved state, `interface_count * CONSERVED_COUNT` entries. */
    const double *left_state;
    /** Right conserved state, same length as `left_state`. */
    const double *right_state;
    /** Physical flux of `left_state`, same length as `left_state`. */
    const double *left_flux;
    /** Physical flux of `right_state`, same length as `left_state`. */
    const double *right_flux;
    /** Leftmost signal speed per interface, `interface_count` entries. */
    const double *left_speed;
    /** Rightmost signal speed per interface, `interface_count` entries. */
    const double *right_speed;
    /** Number of interfaces. */
    size_t interface_count;
} hydrax_kernels_riemann_input;

/**
 * Evaluate the HLL numerical flux at every interface.
 *
 * With `sl` and `sr` the signal speeds bounding the Riemann fan, the flux is
 * the upwind physical flux when the fan does not straddle the interface, and
 * otherwise the HLL average
 *
 *     (sr * fl - sl * fr + sl * sr * (ur - ul)) / (sr - sl).
 *
 * `flux` holds `interface_count * CONSERVED_COUNT` entries.
 *
 * Returns `HYDRAX_KERNELS_ERR_NULL_POINTER` for a null buffer,
 * `HYDRAX_KERNELS_ERR_EMPTY_DOMAIN` for an empty domain,
 * `HYDRAX_KERNELS_ERR_SIZE_OVERFLOW` when the buffer length is not
 * representable, and `HYDRAX_KERNELS_ERR_DEGENERATE_WAVE_SPEEDS` unless every
 * speed pair satisfies `sl <= sr` with both speeds finite.
 */
hydrax_kernels_status
hydrax_kernels_hll_flux(const hydrax_kernels_riemann_input *input, double *flux);

/** Interface fluxes and shell geometry of a spherical grid. */
typedef struct hydrax_kernels_shell_input {
    /**
     * Interface flux, `(cell_count + 1) * CONSERVED_COUNT` entries.
     */
    const double *interface_flux;
    /** Interface area, `cell_count + 1` entries, each finite and >= 0. */
    const double *interface_area;
    /** Cell volume, `cell_count` entries, each finite and > 0. */
    const double *cell_volume;
    /** Source term, `cell_count * CONSERVED_COUNT` entries. */
    const double *source;
    /** Number of cells. */
    size_t cell_count;
} hydrax_kernels_shell_input;

/**
 * Assemble the semi-discrete right-hand side of a spherical finite-volume
 * update.
 *
 * For cell `i` and component `c`,
 *
 *     rhs = source - (area[i + 1] * flux[i + 1] - area[i] * flux[i]) / volume[i].
 *
 * Areas and volumes are supplied rather than derived from a radius, because
 * the discrete divergence is only conservative when the same areas and volumes
 * the grid was built from are the ones used here. Deriving them a second time
 * inside the kernel would be a second, silently divergent, definition of the
 * grid.
 *
 * `rhs` holds `cell_count * CONSERVED_COUNT` entries.
 *
 * Returns `HYDRAX_KERNELS_ERR_NULL_POINTER` for a null buffer,
 * `HYDRAX_KERNELS_ERR_EMPTY_DOMAIN` for `cell_count == 0`,
 * `HYDRAX_KERNELS_ERR_SIZE_OVERFLOW` when the buffer length is not
 * representable, and `HYDRAX_KERNELS_ERR_INVALID_GEOMETRY` for a non-finite or
 * negative area, or a non-finite or non-positive volume.
 */
hydrax_kernels_status
hydrax_kernels_spherical_flux_divergence(const hydrax_kernels_shell_input *input,
                                         double *rhs);

#if defined(__cplusplus)
} /* extern "C" */
#endif

#endif /* HYDRAX_KERNELS_H */
