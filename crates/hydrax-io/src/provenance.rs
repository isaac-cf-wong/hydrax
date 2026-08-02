use std::{fs, path::Path};

use serde::Serialize;

/// The run that produced an output.
#[derive(Debug)]
pub struct RunRecord<'a, C> {
    /// Subcommand or entry point that was executed.
    pub command: &'static str,
    /// Complete configuration of the run, serialized verbatim.
    pub configuration: &'a C,
}

// Derived implementations would demand `C: Clone`/`C: Copy`, but the record
// only borrows the configuration.
impl<C> Clone for RunRecord<'_, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for RunRecord<'_, C> {}

/// The artifact a provenance sidecar describes.
#[derive(Debug, Clone, Copy)]
pub struct OutputRecord {
    /// IANA media type of the artifact.
    pub media_type: &'static str,
    /// Stable schema identifier of the artifact.
    pub schema: &'static str,
}

#[derive(Serialize)]
struct Provenance<'a, C> {
    schema: &'static str,
    software: Software,
    build: Build,
    run: Run<'a, C>,
    output: Output,
}

#[derive(Serialize)]
struct Software {
    name: &'static str,
    version: &'static str,
    repository: &'static str,
    source_revision: &'static str,
    source_state: &'static str,
}

#[derive(Serialize)]
struct Build {
    rustc: &'static str,
    target: &'static str,
    profile: &'static str,
    enabled_features: Vec<&'static str>,
    floating_point: &'static str,
    dependencies: Vec<Dependency>,
}

#[derive(Serialize)]
struct Dependency {
    name: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct Run<'a, C> {
    command: &'static str,
    configuration: &'a C,
}

#[derive(Serialize)]
struct Output {
    media_type: &'static str,
    schema: &'static str,
}

/// Serialize one run and its build metadata to a JSON sidecar.
///
/// The record names the Hydrax workspace rather than the calling crate, so a
/// sidecar identifies the software a reader can check out and rebuild.
///
/// # Errors
///
/// Returns an error if the configuration cannot be serialized or the sidecar
/// cannot be written.
pub fn write_provenance<C: Serialize>(
    path: &Path,
    run: RunRecord<'_, C>,
    output: OutputRecord,
) -> Result<(), Box<dyn std::error::Error>> {
    let provenance = Provenance {
        schema: "hydrax.provenance.v1",
        software: Software {
            name: "hydrax",
            version: env!("CARGO_PKG_VERSION"),
            repository: env!("CARGO_PKG_REPOSITORY"),
            source_revision: env!("HYDRAX_GIT_REVISION"),
            source_state: env!("HYDRAX_SOURCE_STATE"),
        },
        build: Build {
            rustc: env!("HYDRAX_RUSTC_VERSION"),
            target: env!("HYDRAX_BUILD_TARGET"),
            profile: env!("HYDRAX_BUILD_PROFILE"),
            enabled_features: split_nonempty(env!("HYDRAX_BUILD_FEATURES"), ','),
            floating_point: "IEEE 754 binary64",
            dependencies: dependencies(),
        },
        run: Run {
            command: run.command,
            configuration: run.configuration,
        },
        output: Output {
            media_type: output.media_type,
            schema: output.schema,
        },
    };
    let mut json = serde_json::to_vec_pretty(&provenance)?;
    json.push(b'\n');
    fs::write(path, json)?;
    Ok(())
}

/// Decode dependency records embedded by the build script.
fn dependencies() -> Vec<Dependency> {
    split_nonempty(env!("HYDRAX_DEPENDENCIES"), ';')
        .into_iter()
        .filter_map(|entry| {
            let (name, version) = entry.split_once('=')?;
            Some(Dependency { name, version })
        })
        .collect()
}

/// Split a delimited build value while omitting its empty representation.
fn split_nonempty(value: &'static str, separator: char) -> Vec<&'static str> {
    value
        .split(separator)
        .filter(|entry| !entry.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{OutputRecord, RunRecord, write_provenance};
    use serde::Serialize;
    use serde_json::Value;

    #[derive(Serialize)]
    struct Configuration {
        frequency_hz: f64,
    }

    #[test]
    fn sidecar_records_software_run_and_output() {
        let directory = std::env::temp_dir().join(format!(
            "hydrax-io-provenance-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        std::fs::create_dir_all(&directory).expect("temporary directory is creatable");
        let path = directory.join("provenance.json");

        let configuration = Configuration {
            frequency_hz: 800.0,
        };
        write_provenance(
            &path,
            RunRecord {
                command: "test",
                configuration: &configuration,
            },
            OutputRecord {
                media_type: "text/csv",
                schema: "hydrax.test.csv.v1",
            },
        )
        .expect("sidecar is writable");

        let contents = std::fs::read_to_string(&path).expect("sidecar exists");
        let document: Value = serde_json::from_str(&contents).expect("valid JSON");
        assert_eq!(document["schema"], "hydrax.provenance.v1");
        assert_eq!(document["software"]["name"], "hydrax");
        assert_eq!(document["run"]["command"], "test");
        assert_eq!(document["run"]["configuration"]["frequency_hz"], 800.0);
        assert_eq!(document["output"]["schema"], "hydrax.test.csv.v1");
        assert_eq!(document["build"]["floating_point"], "IEEE 754 binary64");

        std::fs::remove_dir_all(&directory).expect("temporary directory is removable");
    }
}
