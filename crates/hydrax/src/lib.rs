//! Core-collapse supernova gravitational-wave simulation tools.
//!
//! Hydrax currently generates the leading-order gravitational waveform from a
//! uniformly sampled mass-quadrupole time series. Input values use SI units.
//!
//! This crate is the stable public surface of the Hydrax workspace. It
//! re-exports three internal layers:
//!
//! - [`hydrax_core`] — units, tensors, and error types,
//! - [`hydrax_wave`] — quadrupole sources, waveforms, spectra, and energy,
//! - [`hydrax_io`] — provenance records for reproducible outputs.
//!
//! Depend on this crate rather than the layers so that internal boundaries can
//! move without breaking callers.

pub use hydrax_core::{
    GRAVITATIONAL_CONSTANT_M3_KG_S2, PARSEC_M, SPEED_OF_LIGHT_M_S, SymmetricTensor3, WaveformError,
};
pub use hydrax_io::{OutputRecord, RunRecord, write_provenance};
pub use hydrax_wave::{
    ComplexAmplitude, EnergyDiagnostics, FiniteDifference, Observer, QuadrupoleSeries,
    ThirdDerivativeStencil, Waveform, WaveformSpectrum, Window, radiated_energy,
    sinusoidal_quadrupole, waveform_from_quadrupole,
};
