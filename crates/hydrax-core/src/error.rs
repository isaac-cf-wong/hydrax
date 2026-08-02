use std::fmt::{Display, Formatter};

/// Errors produced while constructing a source or waveform.
#[derive(Debug, Clone, PartialEq)]
pub enum WaveformError {
    /// A numeric parameter was not finite.
    NonFiniteParameter {
        /// Parameter name.
        name: &'static str,
        /// Rejected value.
        value: f64,
    },
    /// A parameter that must be strictly positive was zero or negative.
    NonPositiveParameter {
        /// Parameter name.
        name: &'static str,
        /// Rejected value.
        value: f64,
    },
    /// A quadrupole sample contained a non-finite component.
    NonFiniteSample {
        /// Index of the rejected sample.
        index: usize,
    },
    /// The selected numerical operation needs more samples.
    TooFewSamples {
        /// Minimum number of samples required.
        required: usize,
        /// Number of samples supplied.
        actual: usize,
    },
    /// A parameter fell outside a closed valid interval.
    ParameterOutOfRange {
        /// Parameter name.
        name: &'static str,
        /// Rejected value.
        value: f64,
        /// Inclusive lower bound.
        minimum: f64,
        /// Inclusive upper bound.
        maximum: f64,
    },
}

impl Display for WaveformError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NonFiniteParameter { name, value } => {
                write!(formatter, "{name} must be finite, got {value}")
            }
            Self::NonPositiveParameter { name, value } => {
                write!(formatter, "{name} must be positive, got {value}")
            }
            Self::NonFiniteSample { index } => {
                write!(formatter, "quadrupole sample {index} is not finite")
            }
            Self::TooFewSamples { required, actual } => write!(
                formatter,
                "at least {required} samples are required, got {actual}"
            ),
            Self::ParameterOutOfRange {
                name,
                value,
                minimum,
                maximum,
            } => write!(
                formatter,
                "{name} must be in [{minimum}, {maximum}], got {value}"
            ),
        }
    }
}

impl std::error::Error for WaveformError {}
