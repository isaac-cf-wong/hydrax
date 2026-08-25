//! Capture reproducibility metadata for provenance sidecars.
//!
//! This script describes the whole Hydrax workspace, not just the crate it
//! belongs to, so it watches every file Git tracks: a sidecar that reports a
//! revision Cargo did not rebuild for is worse than one that reports
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
    let repository = workspace.as_deref().and_then(git_repository);
    if let Some(root) = repository.as_deref() {
        watch_tracked_files(root);
        watch_git(root);
    } else {
        watch_crate_sources(workspace.as_deref());
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

/// Watch every file Git tracks in the repository.
///
/// The watch set is the tracked-file list rather than a walk of the working
/// tree, because tracked files exclude Cargo's build output by construction:
/// nothing here has to recognise that output directory. Recognising it went
/// wrong four times over — by top-level path component, by `CACHEDIR.TAG`, by
/// `.rustc_info.json`, and by excluding a candidate that held tracked source —
/// and each mistake either reported a modified tree as `clean` or reran this
/// script on every build. `git status` is left unfiltered to match: an output
/// directory placed inside the repository and not ignored genuinely is a dirty
/// tree.
///
/// The listing is handled as bytes, one entry at a time. A pathname is a byte
/// string, not text, and a name that is not valid UTF-8 — or that no directive
/// can carry — must cost that one name rather than the whole set: falling back
/// to the crate's own sources over a single awkward filename would leave every
/// other tracked file able to change the reported state without this script
/// running again.
///
/// The one thing not watched at all is a file Git does not track yet. Staging
/// it writes the index, which [`watch_git`] watches, and a new file cannot take
/// part in a build without a tracked manifest or module naming it.
fn watch_tracked_files(root: &Path) {
    let Some(listing) = git_listing(root, &["ls-files", "-z"]) else {
        watch_crate_sources(Some(root));
        return;
    };
    let output = build_output();
    let mut enclosing = HashSet::new();
    let mut watched = false;
    let mut unwatched = 0_usize;
    for entry in listing.split(|byte| *byte == 0) {
        let Some(path) = tracked_path(root, entry) else {
            // Only reachable where a pathname has to be text: a name Git
            // reports that the platform cannot address is left to the warning
            // below rather than silently dropped.
            if !entry.is_empty() {
                unwatched += 1;
            }
            continue;
        };
        let Ok(metadata) = fs::metadata(&path) else {
            // Cargo treats a path it cannot stat as permanently changed, so a
            // tracked file that is gone from the working tree stays out of the
            // set. Its deletion is still seen once, because the build before it
            // was watching the file.
            continue;
        };
        if metadata.is_dir() {
            // Only a submodule is reported as a tracked directory. Cargo walks
            // a watched directory recursively, which would pull the submodule's
            // own build output in and rerun this script on every build.
            continue;
        }
        if emit_watch(&path)
            || watch_enclosing_directory(&path, root, output.as_deref(), &mut enclosing)
        {
            watched = true;
        } else {
            unwatched += 1;
        }
    }
    if !watched {
        watch_crate_sources(Some(root));
    }
    if unwatched > 0 {
        // Staying quiet would leave a tracked file able to change the reported
        // source state without this script running again.
        println!(
            "cargo:warning=hydrax provenance: {unwatched} tracked file(s) have names no build \
             script directive can carry, and no directory above them could stand in; an edit to \
             one of them may leave the recorded source state stale"
        );
    }
}

/// Ask Cargo to watch one file, and report whether it did.
///
/// Cargo treats a watched path it cannot stat as permanently changed, so an
/// absent one reruns this script on every build. Asking the same question Cargo
/// will — metadata through any symlink — keeps that out of the watch set, at no
/// cost: the build before a path vanished was watching it, and a watched path
/// disappearing reruns the script once. Both cases arise here, a tracked file
/// deleted from the working tree and `packed-refs` in a repository whose refs
/// are all still loose.
fn watch(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|metadata| !metadata.is_dir()) && emit_watch(path)
}

/// Watch the nearest directory above `path` that a directive can name.
///
/// Cargo walks a watched directory recursively, so watching the directory that
/// holds a file covers the file too — the only way to watch one whose own name
/// cannot be written. A directory holding Cargo's build output is refused: the
/// build writes there, so watching it would rerun this script every time. Every
/// directory further up holds that output as well, which ends the walk.
fn watch_enclosing_directory(
    path: &Path,
    root: &Path,
    output: Option<&Path>,
    watched: &mut HashSet<PathBuf>,
) -> bool {
    for directory in path.ancestors().skip(1) {
        // The repository root is the outermost directory worth watching; above
        // it lies the rest of the machine.
        if !directory.starts_with(root) {
            return false;
        }
        if output.is_some_and(|output| output.starts_with(directory)) {
            return false;
        }
        if watched.contains(directory) {
            // Already watched, for an earlier file with an equally awkward name.
            return true;
        }
        if emit_watch(directory) {
            watched.insert(directory.to_path_buf());
            return true;
        }
    }
    false
}

/// Locate Cargo's build output, for the one decision that needs to know.
///
/// This is not the output-root detection the watch set used to rest on: no
/// marker file, layout shape or directory name is consulted, and nothing is
/// excluded from `git status`. `OUT_DIR` is simply somewhere inside the output,
/// which is all it takes to refuse to watch a directory that contains it.
fn build_output() -> Option<PathBuf> {
    let out_dir = PathBuf::from(env::var_os("OUT_DIR")?);
    Some(fs::canonicalize(&out_dir).unwrap_or(out_dir))
}

/// Write one watch directive, and report whether the path could be written.
fn emit_watch(path: &Path) -> bool {
    let Some(name) = watchable_name(path) else {
        return false;
    };
    println!("cargo:rerun-if-changed={name}");
    true
}

/// Resolve one pathname from the tracked listing against the repository root.
///
/// A Unix pathname is a byte string, which is why the listing is never decoded
/// as a whole: a name that is not valid UTF-8 still has to be found on disk, so
/// that the directory holding it can be watched in its place.
#[cfg(unix)]
fn tracked_path(root: &Path, name: &[u8]) -> Option<PathBuf> {
    use std::os::unix::ffi::OsStrExt;

    (!name.is_empty()).then(|| root.join(OsStr::from_bytes(name)))
}

/// Resolve one pathname from the tracked listing against the repository root.
///
/// Windows holds a pathname as UTF-16 and Git reports it as UTF-8, so a name
/// that does not survive that round trip cannot be addressed here at all.
#[cfg(not(unix))]
fn tracked_path(root: &Path, name: &[u8]) -> Option<PathBuf> {
    (!name.is_empty())
        .then(|| std::str::from_utf8(name).ok().map(|name| root.join(name)))
        .flatten()
}

/// Return a pathname when a watch directive can carry it.
///
/// Three shapes cannot be carried, and each was measured against Cargo rather
/// than assumed. Cargo decodes a directive as UTF-8 and silently drops a line
/// it cannot decode, so a name that is not valid UTF-8 goes unwatched however
/// it is written — including as raw bytes, which Cargo reads and discards. A
/// directive occupies one line, so a name holding a line feed cannot be written
/// at all. And Cargo strips a carriage return from the end of a line, so a name
/// ending in one would be watched short, which Cargo then treats as permanently
/// changed. A carriage return anywhere else survives.
///
/// [`watch_enclosing_directory`] stands in for all three.
fn watchable_name(path: &Path) -> Option<&str> {
    let name = path.to_str()?;
    (!name.contains('\n') && !name.ends_with('\r')).then_some(name)
}

/// Watch the crate's own sources, for a tree Git cannot enumerate.
///
/// A packaged or vendored copy has no revision to go stale and no working tree
/// to dirty, so the crate's sources and the lockfile behind the recorded
/// dependency list are all that need watching. Both relative paths resolve
/// against this crate's directory, which is what Cargo does with them.
fn watch_crate_sources(workspace: Option<&Path>) {
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-changed=src");
    if let Some(lockfile) = workspace
        .map(|root| root.join("Cargo.lock"))
        .filter(|lockfile| lockfile.is_file())
    {
        println!("cargo:rerun-if-changed={}", lockfile.display());
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
///
/// The loose ref file and `packed-refs` cover each other: only one of them
/// holds the current branch, and packing the refs deletes the loose file the
/// previous build was watching, which reruns this script.
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
        watch(&root.join(resolved));
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
/// Nothing is excluded from `git status`. An exclusion is what manufactured a
/// false `clean` twice, and there is nothing left to exclude: Cargo's build
/// output is either ignored, in which case `git status` passes over it, or it
/// is an unignored directory inside the repository, which is a working tree
/// that really does not match its revision.
fn source_state(repository: Option<&Path>) -> String {
    if let Some(root) = repository {
        return match git_command(root, &["status", "--porcelain", "--untracked-files=normal"])
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

/// Build a Git invocation in a repository.
///
/// `--no-optional-locks` is passed to every Git call: refreshing the index
/// would bump the mtime of a file this script watches, forcing one extra rerun
/// after every fresh checkout.
fn git_command(root: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .arg("--no-optional-locks")
        .args(arguments);
    command
}

/// Run a Git command in a repository and return its trimmed output.
fn git_output(root: &Path, arguments: &[&str]) -> Option<String> {
    trimmed_output(git_command(root, arguments))
}

/// Run a Git command in a repository and return its raw output bytes.
///
/// Used for `-z` listings. Nothing is trimmed or decoded: a pathname may begin
/// or end with whitespace, and it need not be valid UTF-8 at all.
fn git_listing(root: &Path, arguments: &[&str]) -> Option<Vec<u8>> {
    let output = git_command(root, arguments).output().ok()?;
    output.status.success().then_some(output.stdout)
}

/// Run a metadata command and map unavailable output to `unknown`.
fn command_output(directory: Option<&Path>, program: &OsStr, arguments: &[&str]) -> String {
    let mut command = Command::new(program);
    command.args(arguments);
    if let Some(directory) = directory {
        command.current_dir(directory);
    }
    trimmed_output(command).unwrap_or_else(|| "unknown".to_owned())
}

/// Run a command, returning `None` when it fails or prints nothing.
fn trimmed_output(mut command: Command) -> Option<String> {
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
