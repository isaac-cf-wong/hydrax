//! Run configuration, provenance records, and output serialization for Hydrax.
//!
//! Every Hydrax output should be reproducible from its sidecar: the software
//! revision, the toolchain and dependency versions that built it, and the
//! complete run configuration. A run configuration can be written by hand as a
//! TOML file and read back with [`read_configuration`].

mod config;
mod provenance;

pub use config::{ConfigurationError, read_configuration};
pub use provenance::{OutputRecord, RunRecord, write_provenance};
