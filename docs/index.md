# Hydrax

Hydrax is an early-stage Rust package for verified simulations of
gravitational waves from core-collapse supernovae.

The implementation will progress from analytic quadrupole waveforms through
verified collapse hydrodynamics to relativistic multidimensional models. Rust
will provide the safe public API and orchestration, with optional portable C
kernels for performance-critical calculations.

The first time-domain quadrupole waveform API is available for verification.
It is not yet a complete CCSN simulation.

## Local Checks

```shell
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

## Demonstration

```shell
cargo run -- demo > waveform.csv
```

For frequency-domain output:

```shell
cargo run -- demo --output spectrum --window hann > spectrum.csv
```

See the [quadrupole waveform design](design/quadrupole-waveforms.md) for the
equations, conventions, and validity limits.
