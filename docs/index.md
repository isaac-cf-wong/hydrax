# Hydrax

Hydrax is an early-stage Rust package for verified simulations of
gravitational waves from core-collapse supernovae.

The implementation will progress from analytic quadrupole waveforms through
verified collapse hydrodynamics to relativistic multidimensional models. Rust
provides the safe public API and orchestration, with portable C kernels behind
a versioned interface for performance-critical calculations.

The first time-domain quadrupole waveform API is available for verification.
It is not yet a complete CCSN simulation.

## Workspace Layout

The public API lives in the `hydrax` crate, which re-exports three layers:
`hydrax-core` (units, tensors, errors), `hydrax-wave` (quadrupole sources,
waveforms, spectra, energy), and `hydrax-io` (run configuration and
provenance). The `hydrax-cli` crate builds the `hydrax` binary. Depend on
`hydrax` rather than on a layer, so internal boundaries can move without
breaking callers. The private `hydrax-kernels-sys` crate binds the compiled
kernels in `kernels/` and is the only crate that may contain unsafe code.

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

For radiated-energy diagnostics:

```shell
cargo run -- demo --output energy
```

To write a versioned JSON provenance sidecar while streaming CSV data:

```shell
cargo run -- demo --provenance waveform.provenance.json > waveform.csv
```

To collect the settings of a run in a file instead of on the command line:

```shell
cargo run -- demo --config galactic.toml > waveform.csv
```

See [units and numerical conventions](design/units-and-conventions.md) for the
SI and geometrized unit systems and the physical constants.

See the [quadrupole waveform design](design/quadrupole-waveforms.md) for the
equations, conventions, and validity limits.

See [run configuration files](design/run-configuration.md) for the settings a
run accepts and where each one comes from.

See [output formats and provenance](design/output-formats.md) for the CSV
schemas and reproducibility record.

See [the kernel ABI](design/kernel-abi.md) for the boundary between Rust and
the compiled kernels, how it is versioned, and how the kernels are
differentially tested.
