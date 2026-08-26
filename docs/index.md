# Hydrax

Hydrax is an early-stage Rust package for verified simulations of
gravitational waves from core-collapse supernovae.

The implementation will progress from analytic quadrupole waveforms through
verified collapse hydrodynamics to relativistic multidimensional models. Rust
provides the safe public API and orchestration, with portable C kernels behind
a versioned interface for performance-critical calculations.

The first time-domain quadrupole waveform API is available for verification.
It is not yet a complete CCSN simulation.

## Installation

Hydrax targets stable Rust and supports Rust 1.85 or newer, the release that
introduced the 2024 edition. Add the library to a project:

```shell
cargo add hydrax
```

Or install the command-line tool:

```shell
cargo install hydrax-cli
```

Both are published from the same workspace and share a version number. Building
from a checkout needs a C compiler as well, for the kernels in `kernels/`; the
`cc` build script finds the platform's default.

## Versioning

Hydrax follows semantic versioning, with the pre-1.0 convention that a minor
release may make breaking changes. The `hydrax` facade is the surface that
the policy applies to. The layer crates beneath it are published so that `hydrax`
can depend on them, not so that callers can: depend on `hydrax`, and the
internal boundaries can move without breaking you. `hydrax-kernels-sys` is not
published at all.

The kernel ABI carries its own version, independent of the crate version, so a
kernel signature cannot change behind an unchanged crate version. See
[the kernel ABI](design/kernel-abi.md).

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

See [convergence and orders of accuracy](design/convergence.md) for what each
discrete operator approximates, the order it converges at, how that was
measured, and which operators are not yet covered.
