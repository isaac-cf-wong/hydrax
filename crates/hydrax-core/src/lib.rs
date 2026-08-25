//! Units, tensors, numerical conventions, and error types for Hydrax.
//!
//! This crate holds the primitives shared by every Hydrax layer. Public values
//! use SI units unless a name states otherwise. The unit conversions here are
//! the single source of truth for the physical constants and for the
//! geometrized system that sets G = c = 1.

mod error;
mod numeric;
mod tensor;
mod units;

pub use error::WaveformError;
pub use numeric::{index_as_f64, validate_finite, validate_positive};
pub use tensor::SymmetricTensor3;
pub use units::{
    GRAVITATIONAL_CONSTANT_M3_KG_S2, KILOPARSEC_M, PARSEC_M, SOLAR_MASS_KG,
    SOLAR_MASS_PARAMETER_M3_S2, SPEED_OF_LIGHT_M_S, density_from_geometrized_per_m2,
    density_to_geometrized_per_m2, energy_from_geometrized_m, energy_to_geometrized_m,
    kiloparsecs_to_m, m_to_kiloparsecs, m_to_parsecs, mass_from_geometrized_m,
    mass_to_geometrized_m, parsecs_to_m, power_from_geometrized, power_to_geometrized,
    pressure_from_geometrized_per_m2, pressure_to_geometrized_per_m2, speed_from_geometrized,
    speed_to_geometrized, time_from_geometrized_m, time_to_geometrized_m,
};
