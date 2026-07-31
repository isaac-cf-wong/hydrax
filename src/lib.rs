//! Core-collapse supernova gravitational-wave simulation tools.
//!
//! Hydrax currently generates the leading-order gravitational waveform from a
//! uniformly sampled mass-quadrupole time series. Input values use SI units.

mod energy;
mod error;
mod quadrupole;
mod spectrum;
mod tensor;
mod waveform;

pub use energy::{EnergyDiagnostics, ThirdDerivativeStencil, radiated_energy};
pub use error::WaveformError;
pub use quadrupole::{FiniteDifference, QuadrupoleSeries, sinusoidal_quadrupole};
pub use spectrum::{ComplexAmplitude, WaveformSpectrum, Window};
pub use tensor::SymmetricTensor3;
pub use waveform::{Observer, Waveform, waveform_from_quadrupole};

/// Newtonian gravitational constant in m³ kg⁻¹ s⁻².
pub const GRAVITATIONAL_CONSTANT_M3_KG_S2: f64 = 6.674_30e-11;

/// Speed of light in vacuum in m s⁻¹.
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;

/// One parsec in metres, using the IAU 2015 nominal conversion.
pub const PARSEC_M: f64 = 3.085_677_581_491_367e16;
