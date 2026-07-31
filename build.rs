//! Capture reproducibility metadata for provenance sidecars.

use std::{env, fs, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.lock");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src");
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");

    emit(
        "HYDRAX_RUSTC_VERSION",
        command_output("rustc", &["--version"]),
    );
    emit(
        "HYDRAX_GIT_REVISION",
        command_output("git", &["rev-parse", "HEAD"]),
    );
    emit("HYDRAX_SOURCE_STATE", source_state());
    emit(
        "HYDRAX_BUILD_TARGET",
        env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned()),
    );
    emit(
        "HYDRAX_BUILD_PROFILE",
        env::var("PROFILE").unwrap_or_else(|_| "unknown".to_owned()),
    );
    emit("HYDRAX_BUILD_FEATURES", enabled_features());
    emit("HYDRAX_DEPENDENCIES", resolved_dependencies());
}

fn emit(name: &str, value: impl AsRef<str>) {
    println!("cargo:rustc-env={name}={}", value.as_ref());
}

fn command_output(program: &str, arguments: &[&str]) -> String {
    Command::new(program)
        .args(arguments)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|output| output.trim().to_owned())
        .filter(|output| !output.is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

fn source_state() -> String {
    Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=normal"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map_or_else(
            || "unknown".to_owned(),
            |output| {
                if output.stdout.is_empty() {
                    "clean".to_owned()
                } else {
                    "modified".to_owned()
                }
            },
        )
}

fn enabled_features() -> String {
    let mut features = env::vars()
        .filter_map(|(name, _)| name.strip_prefix("CARGO_FEATURE_").map(str::to_owned))
        .map(|name| name.to_ascii_lowercase().replace('_', "-"))
        .collect::<Vec<_>>();
    features.sort_unstable();
    features.join(",")
}

fn resolved_dependencies() -> String {
    let Ok(lockfile) = fs::read_to_string("Cargo.lock") else {
        return String::new();
    };
    let mut dependencies = Vec::new();
    let mut name = None;
    let mut version = None;

    for line in lockfile.lines().chain(["[[package]]"]) {
        if line == "[[package]]" {
            if let (Some(package), Some(package_version)) = (name.take(), version.take()) {
                if package != "hydrax" {
                    dependencies.push(format!("{package}={package_version}"));
                }
            }
        } else if let Some(value) = quoted_value(line, "name") {
            name = Some(value.to_owned());
        } else if let Some(value) = quoted_value(line, "version") {
            version = Some(value.to_owned());
        }
    }

    dependencies.join(";")
}

fn quoted_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.strip_prefix(&format!("{key} = \""))?.strip_suffix('"')
}
