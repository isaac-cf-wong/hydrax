# Hydrax

Hydrax is an early-stage Rust package for verified simulations of
gravitational waves from core-collapse supernovae.

The implementation will progress from analytic quadrupole waveforms through
verified collapse hydrodynamics to relativistic multidimensional models. Rust
will provide the safe public API and orchestration, with optional portable C
kernels for performance-critical calculations.

Hydrax does not yet provide a scientific simulation API.

## Local Checks

```shell
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Current command

```shell
cargo run -- --help
```
