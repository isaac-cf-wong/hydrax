# Hydrax kernels

The compiled numerical kernels behind the Hydrax hydrodynamics solver, and the
C11 header that is the whole of their interface.

| Path                         | Contents                                        |
| ---------------------------- | ----------------------------------------------- |
| `include/hydrax_kernels.h`   | The versioned C ABI: the single source of truth |
| `src/hydrax_kernels.c`       | Version, layout, and status introspection       |
| `src/hydrax_kernels_hydro.c` | Portable CPU implementation of the kernels      |

The Rust side of the boundary is the private `hydrax-kernels-sys` crate, whose
build script compiles these sources. Nothing else in the workspace sees them.

## Why a C ABI

The kernels will eventually run on a GPU, written in CUDA or HIP. Keeping the
interface to C11 means a backend can be written in any of those languages and
stay interchangeable with every other: it exports the symbols this header
declares, with C linkage, and the Rust caller does not change.

The ABI carries its own version, independent of the Hydrax crate version, so
that a change to a kernel signature cannot hide behind an unchanged crate
version. `hydrax_kernels_layout_query` reports the sizes and alignments of the
shared descriptors, which is the drift a version check alone cannot see.

## Conventions

Every floating-point quantity crossing the boundary is a `double`. A state is
stored component-fastest: component `c` of zone `i` lives at index
`i * HYDRAX_KERNELS_CONSERVED_COUNT + c`. Kernels never allocate; every buffer
belongs to the caller.

A kernel validates its whole input before writing anything, so a rejected call
leaves the output buffer untouched. Validation covers the geometry and the
signal speeds — the quantities whose meaning the kernel depends on — and not
the state and flux payloads, whose admissible range is the caller's physics. A
non-finite payload therefore propagates rather than being rejected.

## Adding a kernel

1. Declare it in `include/hydrax_kernels.h` and bump
   `HYDRAX_KERNELS_ABI_VERSION_MINOR`, along with `ABI_VERSION` in
   `crates/hydrax-kernels-sys/src/abi.rs`. The version smoke test fails if only
   one of them moves.
2. Implement it in `src/`, in the same order of floating-point operations as
   the reference below.
3. Add a slow safe-Rust reference to
   `crates/hydrax-kernels-sys/src/reference.rs` and a safe wrapper to
   `crates/hydrax-kernels-sys/src/kernels.rs`.
4. Extend `crates/hydrax-kernels-sys/tests/differential.rs` with a sweep and
   with one case per branch the kernel has.
