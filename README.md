# Hydrax

[![CI](https://github.com/isaac-cf-wong/hydrax/actions/workflows/ci.yml/badge.svg)](https://github.com/isaac-cf-wong/hydrax/actions/workflows/ci.yml)
[![Documentation Status](https://github.com/isaac-cf-wong/hydrax/actions/workflows/documentation.yml/badge.svg)](https://isaac-cf-wong.github.io/hydrax/)
[![Crates.io](https://img.shields.io/crates/v/hydrax)](https://crates.io/crates/hydrax)
[![Docs.rs](https://docs.rs/hydrax/badge.svg)](https://docs.rs/hydrax)
[![License](https://img.shields.io/badge/License-BSD_3--Clause-blue.svg)](LICENSE)

Hydrax is a Rust package for developing verified simulations of gravitational
waves from core-collapse supernovae. It will grow incrementally from analytic
quadrupole waveforms and idealized collapse models toward relativistic
multidimensional radiation hydrodynamics.

Rust will provide the safe public API, simulation orchestration, testing, and
I/O. Performance-critical numerical kernels may be implemented in portable C
behind a small, versioned interface.

> [!IMPORTANT]
> Hydrax is in early development. Its quadrupole waveform API is intended for
> verification and is not yet a complete CCSN simulation. No published results
> should currently be based on it.

## First milestone

The first scientific milestone generates observer-dependent gravitational-wave
polarizations from a prescribed mass-quadrupole time series. The time- and
frequency-domain foundation now includes second- and fourth-order centered
finite differences, explicit Fourier normalization, and configurable window
functions. It is verified against analytic sources, symmetry constraints,
distance scaling, convergence tests, exact discrete spectra, and analytic
radiated-energy calculations. Versioned CSV schemas and JSON provenance
sidecars make demonstration runs machine-readable and auditable.

Later milestones will add spherical hydrodynamics, relativistic collapse and
bounce, axisymmetric rotation, realistic equations of state, and neutrino
transport. Exact spherical symmetry is a verification stage: it emits no
gravitational waves, so the first coupled collapse-and-waveform calculation
requires at least axisymmetry.

## Workspace layout

Hydrax is a Cargo workspace with a narrow boundary between its layers:

| Crate                | Contents                                                    |
| -------------------- | ----------------------------------------------------------- |
| `crates/hydrax`      | Safe public API; re-exports the layers below                |
| `crates/hydrax-cli`  | The `hydrax` binary                                         |
| `crates/hydrax-core` | SI and geometrized units, tensors, conventions, error types |
| `crates/hydrax-wave` | Quadrupole sources, polarizations, spectra, radiated energy |
| `crates/hydrax-io`   | Run configuration, provenance, and output serialization     |

Depend on `hydrax` rather than on a layer directly: the internal boundaries
will move as the physics grows, while the facade stays stable. Every crate
keeps `unsafe_code = "forbid"`; unsafe code is confined to the future
`hydrax-kernels-sys` FFI crate.

## Development

Install the stable Rust toolchain, then run the local quality checks:

```shell
cargo fmt --all --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo doc --no-deps --all-features
```

Generate a demonstration waveform as CSV:

```shell
cargo run -- demo > waveform.csv
```

Generate its nonnegative-frequency spectrum with a Hann window:

```shell
cargo run -- demo --output spectrum --window hann > spectrum.csv
```

Estimate the quadrupole luminosity and total radiated energy:

```shell
cargo run -- demo --output energy
```

Record the complete runtime configuration and build provenance alongside any
of these CSV outputs:

```shell
cargo run -- demo --provenance waveform.provenance.json > waveform.csv
```

The sidecar includes its schema version, the CSV schema identifier, the Hydrax
version and source revision, compiler and target information, enabled features,
resolved dependency versions, floating-point precision, and every demo option.

Collect the settings of a run in a TOML file instead of on the command line:

```toml
# galactic.toml
duration_s = 0.5
frequency_hz = 650.0
distance_kpc = 8.5
output = "spectrum"
window = "tukey"
tukey_alpha = 0.25
```

```shell
cargo run -- demo --config galactic.toml > spectrum.csv
```

A command-line flag overrides one setting of a configuration file without
restating the rest, and an unrecognized key is an error rather than a silently
ignored setting.

The public library API accepts SI units explicitly:

```rust
use hydrax::{
    FiniteDifference, Observer, kiloparsecs_to_m, sinusoidal_quadrupole,
    waveform_from_quadrupole,
};

let source = sinusoidal_quadrupole(
    1.0e38,
    2.0 * std::f64::consts::PI * 800.0,
    0.0,
    1.0 / 16_384.0,
    1024,
)?;
let observer = Observer::new(kiloparsecs_to_m(10.0), 0.0, 0.0)?;
let waveform = waveform_from_quadrupole(
    &source,
    observer,
    FiniteDifference::FivePoint,
)?;
let spectrum = waveform.spectrum(hydrax::Window::Hann)?;
let energy = hydrax::radiated_energy(
    &source,
    hydrax::ThirdDerivativeStencil::SevenPoint,
)?;

assert_eq!(waveform.h_plus().len(), 1020);
assert_eq!(spectrum.h_plus().len(), 511);
assert!(energy.total_energy_j() > 0.0);
# Ok::<(), hydrax::WaveformError>(())
```

Geometrized conversions turn those SI values into the `G = c = 1` system the
relativistic milestones will use internally, and back again.

The unit systems and physical constants are documented in
[units and numerical conventions](docs/design/units-and-conventions.md). The
conventions and validity limits are documented in the
[quadrupole waveform design](docs/design/quadrupole-waveforms.md). The settings
a run accepts are documented in
[run configuration files](docs/design/run-configuration.md), and the stable
column definitions and provenance fields in
[output formats and provenance](docs/design/output-formats.md).

See [CONTRIBUTING.md](CONTRIBUTING.md) for the complete development workflow.

## Documentation

- [Project documentation](https://isaac-cf-wong.github.io/hydrax/)
- [Rust API documentation](https://docs.rs/hydrax)
- [Issue tracker](https://github.com/isaac-cf-wong/hydrax/issues)
- [Discussions](https://github.com/isaac-cf-wong/hydrax/discussions)

## License and citation

Hydrax is distributed under the [3-Clause BSD License](LICENSE). Citation
metadata are provided in [CITATION.cff](CITATION.cff).
