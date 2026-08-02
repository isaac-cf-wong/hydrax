//! Capture reproducibility metadata for provenance sidecars.
//!
//! This script describes the whole Hydrax workspace, not just the crate it
//! belongs to, so it has to watch the whole workspace: a sidecar that reports
//! a revision Cargo did not rebuild for is worse than one that reports
//! `unknown`.

use std::{
    collections::HashSet,
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

/// Entries at the workspace root that must never be watched.
///
/// `.git` is watched precisely by [`watch_git`] instead of wholesale, and
/// `.codegraph` is a local tool cache. The build output directory is excluded
/// separately by [`classify`], because it is not always called `target` and is
/// not always at the workspace root.
const UNWATCHED_ROOT_ENTRIES: [&str; 2] = [".git", ".codegraph"];

/// Watch every workspace file that can change the recorded revision or state.
///
/// The recorded state comes from `git status` over the whole workspace, so the
/// watch set has to be the whole workspace too. Watching only each crate's
/// sources would leave `README.md`, `docs/`, workflow files, and future C
/// sources outside `src/` able to dirty the tree without invalidating this
/// script. Cargo walks watched directories recursively, so files created inside
/// a watched directory are covered as well.
///
/// One gap remains by construction: an entry created at the workspace root
/// itself after a build is not watched, because the root directory cannot be
/// watched without pulling in the build output. Closing it would mean rerunning
/// this script on every build, which costs a relink of every dependent crate.
fn watch_workspace(root: &Path) {
    if fs::read_dir(root).is_err() {
        // Fall back to the crate's own sources rather than silently watching
        // nothing.
        println!("cargo:rerun-if-changed=Cargo.toml");
        println!("cargo:rerun-if-changed=src");
        return;
    }
    let mut visited = HashSet::new();
    watch_directory(root, build_output_dir(root).as_deref(), true, &mut visited);
}

/// Watch every entry of `directory` except Cargo's build output.
///
/// When the output directory sits inside a source directory — a nested
/// `CARGO_TARGET_DIR` such as `crates/hydrax-io/target` — that ancestor cannot
/// simply be skipped, or every crate beneath it would go unwatched. It is
/// descended into instead, so each sibling of the output directory is watched
/// individually and only the output subtree itself is left out.
///
/// `visited` holds the canonical directories already descended into. A symlink
/// pointing at one of its own ancestors would otherwise make this recurse until
/// the build script overflows its stack.
fn watch_directory(
    directory: &Path,
    build_output: Option<&Path>,
    is_root: bool,
    visited: &mut HashSet<PathBuf>,
) {
    if !visited.insert(fs::canonicalize(directory).unwrap_or_else(|_| directory.to_path_buf())) {
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if is_root
            && UNWATCHED_ROOT_ENTRIES
                .iter()
                .any(|unwatched| name == *unwatched)
        {
            continue;
        }
        let path = entry.path();
        match classify(&path, build_output) {
            Disposition::Skip => {}
            Disposition::Descend => watch_directory(&path, build_output, false, visited),
            Disposition::Watch => println!("cargo:rerun-if-changed={}", path.display()),
        }
    }
}

/// What [`watch_directory`] should do with one entry.
enum Disposition {
    /// The entry is Cargo's build output.
    Skip,
    /// The entry contains Cargo's build output somewhere beneath it.
    Descend,
    /// The entry is ordinary source.
    Watch,
}

/// Decide how an entry relates to Cargo's build output.
///
/// Watching the output directory would make the build dirty itself: the script
/// writes into it, so every no-op build would rerun the script.
fn classify(path: &Path, build_output: Option<&Path>) -> Disposition {
    let Some(build_output) = build_output else {
        // Without `OUT_DIR`, fall back to the conventional name.
        if path.file_name().is_some_and(|name| name == "target") {
            return Disposition::Skip;
        }
        return Disposition::Watch;
    };
    let canonical = fs::canonicalize(path);
    let path = canonical.as_deref().unwrap_or(path);
    if build_output == path {
        return Disposition::Skip;
    }
    if build_output.starts_with(path) {
        return Disposition::Descend;
    }
    Disposition::Watch
}

/// Files Cargo writes into the root of a build output directory.
///
/// `CACHEDIR.TAG` is written only when Cargo creates the directory itself, so
/// it is missing whenever the directory already existed. `.rustc_info.json` is
/// written on every build, which covers that case.
const BUILD_OUTPUT_MARKERS: [&str; 2] = ["CACHEDIR.TAG", ".rustc_info.json"];

/// Return the directory Cargo is writing build output into.
///
/// `CARGO_TARGET_DIR` is not forwarded to build scripts, but `OUT_DIR` always
/// sits inside the output directory, and Cargo marks that directory's root.
/// The outermost marked ancestor is taken so that a `--target <triple>`
/// subdirectory does not stand in for the real root.
fn build_output_dir(root: &Path) -> Option<PathBuf> {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR")?);
    let out_dir = fs::canonicalize(&out_dir).unwrap_or(out_dir);
    let canonical_root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    out_dir
        .ancestors()
        // A marker at or above the workspace root would exclude the sources
        // themselves, which is worse than excluding nothing.
        .filter(|ancestor| !canonical_root.starts_with(ancestor))
        .filter(|ancestor| {
            BUILD_OUTPUT_MARKERS
                .iter()
                .any(|marker| ancestor.join(marker).is_file())
        })
        // `Ancestors` runs innermost-first and is not double-ended.
        .last()
        .map(Path::to_path_buf)
}

/// Return a Git exclude pathspec for a build output directory inside `root`.
///
/// Returns `None` when the output directory is outside the repository. The
/// conventional in-tree `target/` is already ignored by `.gitignore`; this
/// matters for a custom `CARGO_TARGET_DIR` placed inside the workspace.
fn build_output_pathspec(root: &Path) -> Option<String> {
    let build_output = build_output_dir(root)?;
    let canonical_root = fs::canonicalize(root).ok()?;
    let relative = build_output.strip_prefix(&canonical_root).ok()?;
    let mut path = String::new();
    for component in relative.components() {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(component.as_os_str().to_str()?);
    }
    if path.is_empty() {
        return None;
    }
    // `literal` disables wildcard interpretation of a directory name Cargo's
    // user chose, and `top` anchors it to the repository root rather than to
    // any same-named directory deeper in the tree.
    Some(format!(":(exclude,literal,top){path}"))
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
        let Some(resolved) = git_output(root, &["rev-parse", "--git-path", &path]) else {
            continue;
        };
        // `--git-path` answers relative to the repository, but Cargo resolves
        // a relative `rerun-if-changed` against this crate's directory. Emit
        // an absolute path so the watch lands on the real file.
        println!("cargo:rerun-if-changed={}", root.join(resolved).display());
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
///
/// A build output directory inside the repository is excluded: it is written
/// by this very build, and reporting the workspace as `modified` because Cargo
/// created its own output would describe the build rather than the source.
fn source_state(repository: Option<&Path>) -> String {
    if let Some(root) = repository {
        let mut arguments = vec![
            "status".to_owned(),
            "--porcelain".to_owned(),
            "--untracked-files=normal".to_owned(),
        ];
        if let Some(excluded) = build_output_pathspec(root) {
            arguments.push("--".to_owned());
            arguments.push(".".to_owned());
            arguments.push(excluded);
        }
        return match Command::new("git")
            .current_dir(root)
            .args(&arguments)
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
///
/// The value is scanned rather than parsed, so it is validated as a full
/// Git object name before use: a malformed or future-shaped record (say
/// `"sha1": null`) would otherwise let the scan run on and return a later
/// quoted value as the revision.
fn packaged_revision() -> Option<String> {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR")?);
    let info = fs::read_to_string(manifest_dir.join(".cargo_vcs_info.json")).ok()?;
    let (_, after) = info.split_once("\"sha1\"")?;
    let (between, after) = after.split_once('"')?;
    // Only whitespace and the key separator may sit before the opening quote.
    if between.trim() != ":" {
        return None;
    }
    let (sha1, _) = after.split_once('"')?;
    is_object_name(sha1).then(|| sha1.to_owned())
}

/// Report whether a string is a full hexadecimal Git object name.
///
/// Cargo still calls the field `sha1`, but it serializes the repository's
/// actual object ID, which is 64 characters in a SHA-256 repository.
fn is_object_name(value: &str) -> bool {
    matches!(value.len(), 40 | 64) && value.chars().all(|character| character.is_ascii_hexdigit())
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
