//! Gravitational waveforms from mass-quadrupole time series.
//!
//! This crate turns a uniformly sampled mass quadrupole into observer-dependent
//! polarizations, spectra, and radiated-energy diagnostics. Input values use SI
//! units.

mod energy;
mod quadrupole;
mod spectrum;
mod waveform;

// `QuadrupoleSeries::new` takes `SymmetricTensor3` values and every fallible
// constructor returns `WaveformError`, so this crate's own API is unusable
// without them.
pub use hydrax_core::{SymmetricTensor3, WaveformError};

pub use energy::{EnergyDiagnostics, ThirdDerivativeStencil, radiated_energy};
pub use quadrupole::{FiniteDifference, QuadrupoleSeries, sinusoidal_quadrupole};
pub use spectrum::{ComplexAmplitude, WaveformSpectrum, Window};
pub use waveform::{Observer, Waveform, waveform_from_quadrupole};
