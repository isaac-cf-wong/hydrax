//! Core-collapse supernova gravitational-wave simulation tools.
//!
//! Hydrax currently generates the leading-order gravitational waveform from a
//! uniformly sampled mass-quadrupole time series. Input values use SI units;
//! the geometrized conversions convert them to and from the G = c = 1 system
//! that the relativistic milestones will use internally.
//!
//! This crate is the stable public surface of the Hydrax workspace. It
//! re-exports three internal layers:
//!
//! - [`hydrax_core`] — units, tensors, and error types,
//! - [`hydrax_wave`] — quadrupole sources, waveforms, spectra, and energy,
//! - [`hydrax_io`] — run configuration and provenance records for reproducible
//!   outputs.
//!
//! Depend on this crate rather than the layers so that internal boundaries can
//! move without breaking callers.

pub use hydrax_core::{
    GRAVITATIONAL_CONSTANT_M3_KG_S2, KILOPARSEC_M, PARSEC_M, SOLAR_MASS_KG,
    SOLAR_MASS_PARAMETER_M3_S2, SPEED_OF_LIGHT_M_S, SymmetricTensor3, WaveformError,
    density_from_geometrized_per_m2, density_to_geometrized_per_m2, energy_from_geometrized_m,
    energy_to_geometrized_m, kiloparsecs_to_m, m_to_kiloparsecs, m_to_parsecs,
    mass_from_geometrized_m, mass_to_geometrized_m, parsecs_to_m, power_from_geometrized,
    power_to_geometrized, pressure_from_geometrized_per_m2, pressure_to_geometrized_per_m2,
    speed_from_geometrized, speed_to_geometrized, time_from_geometrized_m, time_to_geometrized_m,
};
pub use hydrax_io::{
    ConfigurationError, OutputRecord, RunRecord, read_configuration, write_provenance,
};
pub use hydrax_wave::{
    ComplexAmplitude, EnergyDiagnostics, FiniteDifference, Observer, QuadrupoleSeries,
    ThirdDerivativeStencil, Waveform, WaveformSpectrum, Window, radiated_energy,
    sinusoidal_quadrupole, waveform_from_quadrupole,
};
