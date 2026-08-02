//! Capture reproducibility metadata for provenance sidecars.
//!
//! This script describes the whole Hydrax workspace, not just the crate it
//! belongs to, so it has to watch the whole workspace: a sidecar that reports
//! a revision Cargo did not rebuild for is worse than one that reports
//! `unknown`.

use std::{
    env,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Export source, toolchain, target, feature, and dependency metadata.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let workspace = hydrax_workspace_root();
    if let Some(root) = workspace.as_deref() {
        watch_workspace(root);
    } else {
        // Packaged or vendored sources: the crate's own files are all that can
        // be observed.
        println!("cargo:rerun-if-changed=Cargo.toml");
        println!("cargo:rerun-if-changed=src");
    }

    let repository = workspace.as_deref().and_then(git_repository);
    if let Some(root) = &repository {
        watch_git(root);
    }

    let rustc = env::var_os("RUSTC").unwrap_or_else(|| PathBuf::from("rustc").into_os_string());
    emit(
        "HYDRAX_RUSTC_VERSION",
        command_output(None, &rustc, &["--version"]),
    );
    emit(
        "HYDRAX_GIT_REVISION",
        source_revision(repository.as_deref()),
    );
    emit("HYDRAX_SOURCE_STATE", source_state(repository.as_deref()));
    emit(
        "HYDRAX_BUILD_TARGET",
        env::var("TARGET").unwrap_or_else(|_| "unknown".to_owned()),
    );
    emit(
        "HYDRAX_BUILD_PROFILE",
        env::var("PROFILE").unwrap_or_else(|_| "unknown".to_owned()),
    );
    emit("HYDRAX_BUILD_FEATURES", enabled_features());
    emit(
        "HYDRAX_DEPENDENCIES",
        resolved_dependencies(workspace.as_deref()),
    );
}

/// Expose one metadata value to the package compilation.
fn emit(name: &str, value: impl AsRef<str>) {
    println!("cargo:rustc-env={name}={}", value.as_ref());
}

/// Locate the Hydrax workspace root above this crate.
///
/// The walk stops at a directory that is positively identifiable as the Hydrax
/// workspace, so a vendored copy cannot adopt a surrounding project's manifest
/// or lockfile.
fn hydrax_workspace_root() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
    manifest_dir
        .ancestors()
        .find(|directory| is_hydrax_workspace(directory))
        .map(Path::to_path_buf)
}

/// Report whether a directory is the root of the Hydrax workspace.
fn is_hydrax_workspace(directory: &Path) -> bool {
    let Ok(manifest) = fs::read_to_string(directory.join("Cargo.toml")) else {
        return false;
    };
    manifest
        .lines()
        .any(|line| line.trim_end() == "[workspace]")
        && directory.join("crates/hydrax-core/Cargo.toml").is_file()
}

/// Watch every workspace source that can change the recorded revision.
///
/// Watching only this crate would let an incremental rebuild reuse a stale
/// sidecar after another crate changed.
fn watch_workspace(root: &Path) {
    for relative in ["Cargo.toml", "Cargo.lock"] {
        println!("cargo:rerun-if-changed={}", root.join(relative).display());
    }
    let Ok(entries) = fs::read_dir(root.join("crates")) else {
        return;
    };
    for entry in entries.flatten() {
        let crate_dir = entry.path();
        if !crate_dir.is_dir() {
            continue;
        }
        for relative in ["Cargo.toml", "src", "build.rs"] {
            let path = crate_dir.join(relative);
            if path.exists() {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
}

/// Return the workspace root when it is itself the root of a Git repository.
///
/// A workspace nested inside an unrelated repository is deliberately treated
/// as having no revision: recording the surrounding project's commit under
/// Hydrax's repository URL would be a false provenance claim.
fn git_repository(root: &Path) -> Option<PathBuf> {
    let toplevel = git_output(root, &["rev-parse", "--show-toplevel"])?;
    let toplevel = PathBuf::from(toplevel);
    (fs::canonicalize(&toplevel).ok()? == fs::canonicalize(root).ok()?).then_some(toplevel)
}

/// Watch the Git files that change when the checked-out revision changes.
///
/// `--git-path` is used rather than assuming `<root>/.git/...`, because a
/// linked worktree stores `.git` as a file pointing elsewhere.
fn watch_git(root: &Path) {
    let mut paths = vec![
        "HEAD".to_owned(),
        "index".to_owned(),
        "packed-refs".to_owned(),
    ];
    // A commit on the current branch leaves HEAD's symbolic ref untouched and
    // only rewrites the branch's own ref file.
    if let Some(reference) = git_output(root, &["symbolic-ref", "--quiet", "HEAD"]) {
        paths.push(reference);
    }
    for path in paths {
        if let Some(resolved) = git_output(root, &["rev-parse", "--git-path", &path]) {
            println!("cargo:rerun-if-changed={resolved}");
        }
    }
}

/// Return the revision the sources were built from.
fn source_revision(repository: Option<&Path>) -> String {
    repository
        .and_then(|root| git_output(root, &["rev-parse", "HEAD"]))
        .or_else(packaged_revision)
        .unwrap_or_else(|| "unknown".to_owned())
}

/// Report whether the working tree matched its revision at build time.
fn source_state(repository: Option<&Path>) -> String {
    if let Some(root) = repository {
        return match Command::new("git")
            .current_dir(root)
            .args(["status", "--porcelain", "--untracked-files=normal"])
            .output()
        {
            Ok(output) if output.status.success() && output.stdout.is_empty() => "clean".to_owned(),
            Ok(output) if output.status.success() => "modified".to_owned(),
            _ => "unknown".to_owned(),
        };
    }
    if packaged_revision().is_some() {
        // A published .crate is a snapshot; there is no working tree to dirty.
        return "packaged".to_owned();
    }
    "unknown".to_owned()
}

/// Read the revision Cargo recorded when packaging this crate.
fn packaged_revision() -> Option<String> {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
    let info = fs::read_to_string(manifest_dir.join(".cargo_vcs_info.json")).ok()?;
    let (_, after) = info.split_once("\"sha1\"")?;
    let (_, after) = after.split_once('"')?;
    let (sha1, _) = after.split_once('"')?;
    (!sha1.is_empty()).then(|| sha1.to_owned())
}

/// Run a Git command in a repository and return its trimmed output.
fn git_output(root: &Path, arguments: &[&str]) -> Option<String> {
    command_output_opt(Some(root), OsStr::new("git"), arguments)
}

/// Run a metadata command and map unavailable output to `unknown`.
fn command_output(directory: Option<&Path>, program: &OsStr, arguments: &[&str]) -> String {
    command_output_opt(directory, program, arguments).unwrap_or_else(|| "unknown".to_owned())
}

/// Run a metadata command, returning `None` when it fails or prints nothing.
fn command_output_opt(
    directory: Option<&Path>,
    program: &OsStr,
    arguments: &[&str],
) -> Option<String> {
    let mut command = Command::new(program);
    command.args(arguments);
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    command
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|output| output.trim().to_owned())
        .filter(|output| !output.is_empty())
}

/// Return a stable comma-separated list of enabled Cargo features.
fn enabled_features() -> String {
    let mut features = env::vars()
        .filter_map(|(name, _)| name.strip_prefix("CARGO_FEATURE_").map(str::to_owned))
        .map(|name| name.to_ascii_lowercase().replace('_', "-"))
        .collect::<Vec<_>>();
    features.sort_unstable();
    features.join(",")
}

/// Return resolved package names and versions from the Cargo lockfile.
fn resolved_dependencies(workspace: Option<&Path>) -> String {
    let Some(lockfile) = workspace
        .map(|root| root.join("Cargo.lock"))
        .or_else(|| Some(PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?).join("Cargo.lock")))
        .and_then(|path| fs::read_to_string(path).ok())
    else {
        return String::new();
    };
    let mut dependencies = Vec::new();
    let mut name: Option<String> = None;
    let mut version: Option<String> = None;

    for line in lockfile.lines().chain(["[[package]]"]) {
        if line == "[[package]]" {
            if let (Some(package), Some(package_version)) = (name.take(), version.take()) {
                // Workspace members share the crate version already recorded
                // under `software`, so only external dependencies are listed.
                if package != "hydrax" && !package.starts_with("hydrax-") {
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

/// Extract a quoted TOML scalar from a Cargo lockfile line.
fn quoted_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.strip_prefix(&format!("{key} = \""))?.strip_suffix('"')
}
