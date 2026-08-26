# hydrax-kernels-sys

Versioned C ABI bindings to the [Hydrax](https://github.com/isaac-cf-wong/hydrax)
numerical kernels.

This crate is private to the workspace and is not published. It is the only
crate in which `unsafe` is permitted; every other crate sets
`unsafe_code = "forbid"`, which is a meaningful statement about the project
only because the exception is confined here.

It holds the declarations of the C entry points in `kernels/`, the safe
wrappers that check what the ABI leaves to the caller, the version and layout
checks that decide whether a compiled backend may be used, and a slow safe-Rust
reference for every kernel so that the compiled implementation can be
differentially tested against one short enough to read.
