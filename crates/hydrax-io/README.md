# hydrax-io

Run configuration, provenance records, and output serialization for
[Hydrax](https://github.com/isaac-cf-wong/hydrax).

This crate reads the settings of a run from a TOML file and captures the build,
source, and configuration metadata that every Hydrax output must record. It is
an implementation detail of the `hydrax` public API.
