//! Units, tensors, numerical conventions, and error types for Hydrax.
//!
//! This crate holds the primitives shared by every Hydrax layer. Public values
//! use SI units unless a name states otherwise.

mod error;
mod numeric;
mod tensor;

pub use error::WaveformError;
pub use numeric::{index_as_f64, validate_finite, validate_positive};
pub use tensor::SymmetricTensor3;

/// Newtonian gravitational constant in m³ kg⁻¹ s⁻².
pub const GRAVITATIONAL_CONSTANT_M3_KG_S2: f64 = 6.674_30e-11;

/// Speed of light in vacuum in m s⁻¹.
pub const SPEED_OF_LIGHT_M_S: f64 = 299_792_458.0;

/// One parsec in metres, using the IAU 2015 nominal conversion.
pub const PARSEC_M: f64 = 3.085_677_581_491_367e16;
