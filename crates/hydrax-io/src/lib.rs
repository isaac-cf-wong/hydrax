//! Provenance records and output serialization for Hydrax.
//!
//! Every Hydrax output should be reproducible from its sidecar: the software
//! revision, the toolchain and dependency versions that built it, and the
//! complete run configuration.

mod provenance;

pub use provenance::{OutputRecord, RunRecord, write_provenance};
