use std::fmt::{Display, Formatter};

/// A status code the kernel ABI defines.
///
/// The numeric codes are written out here rather than derived from the C
/// header, so that a code renumbered on one side of the boundary and not the
/// other is a test failure rather than a silent relabelling of an error. The
/// version smoke test checks this table against the library's own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KernelStatus {
    /// The call succeeded.
    Ok,
    /// A required buffer pointer was null.
    NullPointer,
    /// The domain held no zones.
    EmptyDomain,
    /// The caller and the library disagree about the ABI.
    AbiMismatch,
    /// A cell volume or interface area was not a usable geometric measure.
    InvalidGeometry,
    /// A signal-speed pair was not a finite, correctly ordered interval.
    DegenerateWaveSpeeds,
    /// The requested domain size cannot be addressed on this target.
    SizeOverflow,
}

impl KernelStatus {
    /// Every status the ABI defines, in code order.
    pub const ALL: [Self; 7] = [
        Self::Ok,
        Self::NullPointer,
        Self::EmptyDomain,
        Self::AbiMismatch,
        Self::InvalidGeometry,
        Self::DegenerateWaveSpeeds,
        Self::SizeOverflow,
    ];

    /// Return the code the ABI assigns to this status.
    #[must_use]
    pub const fn code(self) -> i32 {
        match self {
            Self::Ok => 0,
            Self::NullPointer => 1,
            Self::EmptyDomain => 2,
            Self::AbiMismatch => 3,
            Self::InvalidGeometry => 4,
            Self::DegenerateWaveSpeeds => 5,
            Self::SizeOverflow => 6,
        }
    }

    /// Return the status a code names, or `None` for a code this crate does
    /// not know.
    #[must_use]
    pub const fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Ok),
            1 => Some(Self::NullPointer),
            2 => Some(Self::EmptyDomain),
            3 => Some(Self::AbiMismatch),
            4 => Some(Self::InvalidGeometry),
            5 => Some(Self::DegenerateWaveSpeeds),
            6 => Some(Self::SizeOverflow),
            _ => None,
        }
    }
}

impl Display for KernelStatus {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        let description = match self {
            Self::Ok => "the call succeeded",
            Self::NullPointer => "a required buffer pointer was null",
            Self::EmptyDomain => "the domain held no zones",
            Self::AbiMismatch => "the caller and the kernel library disagree about the ABI",
            Self::InvalidGeometry => {
                "a cell volume or interface area was not a usable geometric measure"
            }
            Self::DegenerateWaveSpeeds => {
                "a signal-speed pair was not a finite, correctly ordered interval"
            }
            Self::SizeOverflow => "the requested domain size cannot be addressed on this target",
        };
        formatter.write_str(description)
    }
}

/// Errors produced while calling a kernel.
///
/// The safe-Rust reference implementations return the same errors as the
/// compiled kernels, so a differential test can compare rejections as directly
/// as it compares numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KernelError {
    /// A buffer length did not match the domain the call described.
    LengthMismatch {
        /// Name of the buffer, as the ABI calls it.
        buffer: &'static str,
        /// Length the domain size implies.
        expected: usize,
        /// Length supplied.
        actual: usize,
    },
    /// A kernel rejected the call.
    Rejected(KernelStatus),
    /// A kernel returned a status this crate does not define.
    ///
    /// Reaching this means the compiled library is newer than the crate in a
    /// way the version check let through, which is a defect in one of them.
    UnknownStatus(i32),
    /// The compiled library does not implement the ABI this crate expects.
    Incompatible {
        /// Version this crate was written against.
        expected: crate::AbiVersion,
        /// Version the library reports.
        found: crate::AbiVersion,
    },
}

impl KernelError {
    /// Map a raw status code to a result.
    ///
    /// # Errors
    ///
    /// Returns [`KernelError::Rejected`] for a defined failure and
    /// [`KernelError::UnknownStatus`] for a code this crate does not define.
    pub fn from_status(code: i32) -> Result<(), Self> {
        match KernelStatus::from_code(code) {
            Some(KernelStatus::Ok) => Ok(()),
            Some(status) => Err(Self::Rejected(status)),
            None => Err(Self::UnknownStatus(code)),
        }
    }
}

impl Display for KernelError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LengthMismatch {
                buffer,
                expected,
                actual,
            } => write!(
                formatter,
                "{buffer} must hold {expected} values, got {actual}"
            ),
            Self::Rejected(status) => write!(formatter, "the kernel rejected the call: {status}"),
            Self::UnknownStatus(code) => {
                write!(formatter, "the kernel returned undefined status {code}")
            }
            Self::Incompatible { expected, found } => write!(
                formatter,
                "the kernel library implements ABI {found}, but this crate needs {expected}"
            ),
        }
    }
}

impl std::error::Error for KernelError {}

#[cfg(test)]
mod tests {
    use super::{KernelError, KernelStatus};

    #[test]
    fn every_status_round_trips_through_its_code() {
        for status in KernelStatus::ALL {
            assert_eq!(KernelStatus::from_code(status.code()), Some(status));
        }
    }

    #[test]
    fn status_codes_are_distinct() {
        // Written as a pairwise sweep rather than a set, so a duplicate names
        // both statuses that collided.
        for (position, status) in KernelStatus::ALL.iter().enumerate() {
            for other in &KernelStatus::ALL[position + 1..] {
                assert_ne!(
                    status.code(),
                    other.code(),
                    "{status:?} and {other:?} share a code"
                );
            }
        }
    }

    #[test]
    fn undefined_codes_have_no_status() {
        for code in [-1, 7, 8, i32::MIN, i32::MAX] {
            assert_eq!(KernelStatus::from_code(code), None);
            assert_eq!(
                KernelError::from_status(code),
                Err(KernelError::UnknownStatus(code))
            );
        }
    }

    #[test]
    fn success_is_the_only_status_that_is_not_an_error() {
        assert_eq!(KernelError::from_status(KernelStatus::Ok.code()), Ok(()));
        for status in KernelStatus::ALL {
            if status == KernelStatus::Ok {
                continue;
            }
            assert_eq!(
                KernelError::from_status(status.code()),
                Err(KernelError::Rejected(status))
            );
        }
    }
}
