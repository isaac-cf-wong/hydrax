# The kernel ABI

## Scope

Hydrax solves relativistic hydrodynamics in Rust and hands the innermost loops
to compiled kernels. This page describes the boundary between the two: what
crosses it, how it is versioned, and how a compiled kernel is shown to compute
what it claims to.

The boundary is one header, `kernels/include/hydrax_kernels.h`, and one crate,
`hydrax-kernels-sys`. Nothing else in the workspace sees a raw pointer.

## Why C11 rather than C++ or CUDA

The kernels that matter for a core-collapse run will end up on a GPU, written
in CUDA or HIP. Declaring the interface in C11 is what keeps that from becoming
a rewrite: a backend written in C, C++, CUDA, or HIP exports the same symbols
with C linkage and is interchangeable with every other. The declarations use no
construct that is not plain C11, so the header can be included from any of
them.

The alternative — binding Rust to C++ directly — would tie the workspace to one
compiler's name mangling and one library's class layout, and would put the
choice of GPU vendor inside the Rust code rather than behind an interface.

## Unsafe code lives in exactly one crate

Every crate in the workspace sets `unsafe_code = "forbid"` except
`hydrax-kernels-sys`, which is where the ABI is declared and called. The
prohibition elsewhere is only a meaningful statement about the project if the
exception is small and stays small, so the crate holds nothing but the
boundary: no physics, no orchestration, no I/O.

Within the crate the exception is narrowed further. `unsafe_op_in_unsafe_fn` is
denied, so an unsafe function is not itself an unsafe block, and
`undocumented_unsafe_blocks` is warned, so every block that is one carries the
argument for why it is sound. Each such argument rests on a length check made
in safe code immediately above it.

## What the version protects

The ABI carries a version of its own, `major.minor.patch`, independent of the
Hydrax crate version. A crate release that leaves the kernels alone must not
force every backend to be rebuilt, and a change to a kernel signature must not
be able to hide behind an unchanged crate version.

| Component | Bump it for                                                                                               |
| --------- | --------------------------------------------------------------------------------------------------------- |
| major     | A removed entry point, a changed signature, a reordered or resized descriptor field, a reused status code |
| minor     | A new entry point or a new status code                                                                    |
| patch     | A behaviour-preserving fix inside an existing kernel                                                      |

A library at version `L` serves a caller compiled against `C` when
`L.major == C.major` and `L.minor >= C.minor`: the library may have gained
entry points the caller does not know about, but it may not be missing one the
caller does. The patch component is not consulted.

## Three statements of the contract

A version number only helps if someone remembers to change it, so the contract
is stated three times and the three are checked against each other. None is
derived from another; a value derived from another agrees with it by
construction and proves nothing.

1. **The crate.** `ABI_VERSION` and `CONSERVED_COUNT` in
   `hydrax-kernels-sys`, written by hand in Rust.
2. **The header.** The same values as C macros, written by hand, read back out
   of the header by the build script and exposed as `hydrax_kernels_sys::header`.
3. **The library.** What the compiled backend reports at run time, through
   `hydrax_kernels_abi_version` and `hydrax_kernels_layout_query`.

`crates/hydrax-kernels-sys/tests/abi_version.rs` asserts they agree. It also
compares the library's own compatibility decision against the rule above, and
its table of status codes against the crate's, so a rule or a code that changed
on one side is a test failure rather than a silent relabelling.

The layout query is what a version check cannot replace. A descriptor can gain
a field, or change alignment on one target, without anyone thinking to bump the
version; the query reports the sizes and alignments the library was compiled
with, and the test compares them against the sizes and alignments of the
mirrored `#[repr(C)]` types. It also compares `sizeof(size_t)` against
`size_of::<usize>()`, because every length crossing the boundary is formed from
a Rust slice.

## Differential testing against a slow reference

A compiled kernel cannot be trusted on inspection. It is written in another
language, compiled with flags the reader did not choose, and will eventually
run on hardware with its own arithmetic. It can be trusted when it reproduces
an implementation short enough to read in one sitting.

Every kernel therefore has a twin in `hydrax_kernels_sys::reference`: safe
Rust, unhurried, with the same signature and the same errors. The differential
tests run both over swept inputs and over one case per branch, and require
agreement to within about forty-five units in the last place.

The tolerance is not slack in the comparison. The two implementations perform
the same operations in the same order, and the C is compiled with
floating-point contraction disabled, so in practice they agree bit for bit. The
tolerance exists so that a compiler which contracts anyway reports a rounding
difference as agreement, while any change to the algorithm — which moves a
result by far more — still fails. Zero must be reproduced exactly, since the
tolerance is relative.

Rejections are compared as directly as numbers: the reference returns the same
errors as the kernel, so a differential case can assert that both accept or
both refuse. The tests also assert that a refused call leaves the output buffer
untouched, including when the offending value is on the last zone of the
domain — a kernel that validated and computed in one pass would already have
written most of the buffer.

This is the harness a GPU backend will be checked with. It does not know which
backend it is testing.

## The kernels

The first set covers the spatial discretization of spherical relativistic
hydrodynamics. Every state and flux buffer carries three conserved variables
per zone — the conserved rest-mass density, the radial momentum density, and
the rescaled energy density of the Valencia formulation — stored
component-fastest.

| Kernel                      | Computes                                                              |
| --------------------------- | --------------------------------------------------------------------- |
| `minmod_slopes`             | Minmod-limited undivided differences of a cell-centred scalar field   |
| `hll_flux`                  | The HLL numerical flux at every interface                             |
| `spherical_flux_divergence` | The semi-discrete right-hand side of a spherical finite-volume update |

Two choices in that table are worth stating, because both keep a duplicated
definition out of the interface. The limiter returns undivided differences, so
the grid's cell widths stay with the grid rather than being restated in the
ABI. The divergence takes interface areas and cell volumes rather than a
radius: the discrete divergence is conservative only when the areas and volumes
used here are the ones the grid was built from, and deriving them a second time
inside the kernel would be a second, silently divergent, definition of the
grid.

## Validation and what is left to the caller

A kernel validates the geometry and the signal speeds — the quantities whose
meaning it depends on. A cell volume must be finite and positive, an interface
area finite and non-negative, and a signal-speed pair finite and correctly
ordered. Validation runs over the whole input before anything is written.

State and flux payloads are not validated. Their admissible range is the
caller's physics rather than the kernel's, and a kernel that rejected a
non-finite density would be making a decision that belongs to the equation of
state. A non-finite payload therefore propagates to the output, identically in
the compiled kernel and in the reference.

Buffer lengths are the Rust wrapper's responsibility, not the ABI's. The safe
wrappers take slices, derive the domain size from one of them, and check every
other against it before forming a pointer — which is what makes the `unsafe`
blocks sound.
