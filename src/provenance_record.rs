use std::{fs, path::Path};

use serde::Serialize;

use crate::DemoArgs;

#[derive(Serialize)]
struct Provenance<'a> {
    schema: &'static str,
    software: Software,
    build: Build,
    run: Run<'a>,
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
struct Run<'a> {
    command: &'static str,
    configuration: &'a DemoArgs,
}

#[derive(Serialize)]
struct Output {
    media_type: &'static str,
    schema: &'static str,
}

/// Serialize one demo run and its build metadata to a JSON sidecar.
pub fn write(
    path: &Path,
    configuration: &DemoArgs,
    output_schema: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    let provenance = Provenance {
        schema: "hydrax.provenance.v1",
        software: Software {
            name: env!("CARGO_PKG_NAME"),
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
            command: "demo",
            configuration,
        },
        output: Output {
            media_type: "text/csv",
            schema: output_schema,
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
