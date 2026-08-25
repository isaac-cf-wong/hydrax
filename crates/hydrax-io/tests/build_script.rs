//! End-to-end checks of the watch set the provenance build script emits.
//!
//! Two failures matter here and neither has ever shown up in a unit test. A
//! script that reruns when nothing changed relinks every dependent crate on
//! every build; a script that fails to rerun stamps outputs with a revision or
//! a state that is no longer true. Both have shipped, and both were caught by
//! a reader rather than by the suite.
//!
//! Each check therefore builds a miniature Hydrax workspace — the real build
//! script, copied in — and reads back Cargo's own record of the run: the
//! record's modification time says whether the script ran, and its contents say
//! what the script watched and reported.
//!
//! Nothing here may run a bare `git status` against a fixture: that refreshes
//! the index the script watches and costs one rerun, which would look like the
//! defect these checks exist to catch.
//!
//! The record is read as bytes throughout, because a pathname is a byte string:
//! the checks below deliberately track files whose names are not valid UTF-8 or
//! hold a line break, and decoding the record as text would fail on the first
//! of them.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::SystemTime,
};

/// Manifest of the fixture workspace root.
const WORKSPACE_MANIFEST: &str = "[workspace]\nmembers = [\"crates/*\"]\nresolver = \"3\"\n";

/// The two directory names Hydrax ignores, so a fixture matches the real tree.
const IGNORED: &str = "debug\ntarget\n";

/// Library that consumes the build script's metadata, as the real crate does.
const LIBRARY: &str = "pub const REVISION: &str = env!(\"HYDRAX_GIT_REVISION\");\n\
                       pub const STATE: &str = env!(\"HYDRAX_SOURCE_STATE\");\n";

/// A throwaway Git repository shaped like the Hydrax workspace.
struct Workspace {
    /// Root of the fixture, holding both its sources and its Git directory.
    root: PathBuf,
}

impl Workspace {
    /// Create and commit the fixture for one check.
    fn create(name: &str) -> Self {
        // A canonical base directory keeps the paths the script emits
        // comparable with the fixture's own, whatever `TMPDIR` points through.
        let base = std::env::temp_dir();
        let base = fs::canonicalize(&base).unwrap_or(base);
        let root = base.join(format!(
            "hydrax-io-build-script-{}-{name}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        let workspace = Self { root };
        workspace.write("Cargo.toml", WORKSPACE_MANIFEST);
        workspace.write(".gitignore", IGNORED);
        workspace.write("README.md", "A fixture workspace.\n");
        workspace.write("crates/hydrax-core/Cargo.toml", &manifest("hydrax-core"));
        workspace.write("crates/hydrax-core/src/lib.rs", "pub fn placeholder() {}\n");
        workspace.write("crates/hydrax-io/Cargo.toml", &manifest("hydrax-io"));
        workspace.write("crates/hydrax-io/src/lib.rs", LIBRARY);
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("build.rs");
        let copied = workspace.path("crates/hydrax-io/build.rs");
        fs::copy(&script, &copied).expect("the build script under test is copyable");

        // Committing the lockfile keeps a built fixture clean; generating it
        // with the same Cargo that builds it keeps Cargo from rewriting it.
        workspace.cargo(
            &["generate-lockfile", "--offline"],
            &workspace.path("target"),
            &[],
        );
        workspace.git(&["init", "--quiet", "--initial-branch=main"]);
        workspace.track_all("Add the fixture workspace");
        workspace
    }

    /// Absolute path of one entry in the fixture.
    fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    /// Write one file, creating the directories above it.
    fn write(&self, relative: &str, contents: &str) {
        let path = self.path(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture directory is creatable");
        }
        fs::write(&path, contents).expect("fixture file is writable");
    }

    /// Write one file whose name is an arbitrary byte string, and track it.
    ///
    /// The name is given as bytes because these are exactly the names that
    /// cannot be spelled as text: a byte that is not valid UTF-8, a line break,
    /// a trailing carriage return.
    #[cfg(unix)]
    fn write_bytes(&self, name: &[u8], contents: &str) -> PathBuf {
        use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

        let path = self.root.join(OsStr::from_bytes(name));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("fixture directory is creatable");
        }
        fs::write(&path, contents).expect("fixture file is writable");
        path
    }

    /// Stage and commit everything in the fixture, as a later edit needs.
    fn track_all(&self, message: &str) {
        self.git(&["add", "--all"]);
        self.commit(message);
    }

    /// Build the fixture and read Cargo's record of the build script's run.
    fn build(&self, target: &Path, environment: &[(&str, &str)]) -> Record {
        self.cargo(
            &["build", "--package", "hydrax-io", "--offline", "--quiet"],
            target,
            environment,
        );
        Record::read(target)
    }

    /// Run Cargo against the fixture with a chosen output directory.
    fn cargo(&self, arguments: &[&str], target: &Path, environment: &[(&str, &str)]) {
        let program = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let mut command = Command::new(program);
        command
            .current_dir(&self.root)
            .args(arguments)
            // Whatever the outer build was configured with must not leak in.
            .env_remove("CARGO_CACHE_RUSTC_INFO")
            .env("CARGO_TARGET_DIR", target)
            // These checks run in parallel and the fixture is two small
            // crates, so one job each keeps the machine from being swamped.
            .env("CARGO_BUILD_JOBS", "1");
        for (name, value) in environment {
            command.env(name, value);
        }
        run(command, "cargo");
    }

    /// Run Git against the fixture.
    fn git(&self, arguments: &[&str]) {
        let mut command = Command::new("git");
        command.current_dir(&self.root).args(arguments);
        run(command, "git");
    }

    /// Commit the staged fixture, independently of the caller's Git settings.
    fn commit(&self, message: &str) {
        self.git(&[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "--no-verify",
            "--allow-empty",
            "--message",
            message,
        ]);
    }

    /// Delete the fixture, which is left behind when a check fails.
    fn remove(self) {
        fs::remove_dir_all(self.root).expect("fixture is removable");
    }
}

/// Manifest of one fixture crate.
fn manifest(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
}

/// Run a command, reporting its diagnostics when it fails.
fn run(mut command: Command, program: &str) {
    let output = command.output().expect("the fixture toolchain is runnable");
    assert!(
        output.status.success(),
        "{program} {:?} failed: {}",
        command.get_args().collect::<Vec<_>>(),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Cargo's record of one run of the build script.
struct Record {
    /// Every directive the script printed, verbatim.
    directives: Vec<u8>,
    /// When the script wrote them.
    written: SystemTime,
}

impl Record {
    /// Read the record Cargo keeps under an output directory.
    fn read(target: &Path) -> Self {
        let builds = target.join("debug").join("build");
        let mut records = fs::read_dir(&builds)
            .expect("Cargo created a build directory")
            .flatten()
            .map(|entry| entry.path().join("output"))
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        assert_eq!(
            records.len(),
            1,
            "exactly one build script record under {}",
            builds.display()
        );
        let path = records.remove(0);
        let written = fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .expect("the record has a modification time");
        let directives = fs::read(&path).expect("the record is readable");
        Self {
            directives,
            written,
        }
    }

    /// Lines of the record.
    fn lines(&self) -> impl Iterator<Item = &[u8]> {
        self.directives.split(|byte| *byte == b'\n')
    }

    /// Value the script exported under `name`.
    fn value(&self, name: &str) -> &str {
        let prefix = format!("cargo:rustc-env={name}=");
        self.lines()
            .find_map(|line| line.strip_prefix(prefix.as_bytes()))
            .and_then(|value| std::str::from_utf8(value).ok())
            .unwrap_or_else(|| panic!("the script exported {name}"))
    }

    /// Pathnames the script asked Cargo to watch, as it wrote them.
    fn watched(&self) -> impl Iterator<Item = &[u8]> {
        self.lines()
            .filter_map(|line| line.strip_prefix(b"cargo:rerun-if-changed=".as_slice()))
    }

    /// How many times the script asked Cargo to watch exactly `path`.
    fn watch_count(&self, path: &Path) -> usize {
        self.watched()
            .filter(|watched| path_of(watched).is_some_and(|watched| watched == path))
            .count()
    }

    /// Whether the script asked Cargo to watch exactly `path`.
    fn watches(&self, path: &Path) -> bool {
        self.watch_count(path) > 0
    }

    /// Whether the script asked Cargo to watch anything inside `directory`.
    fn watches_inside(&self, directory: &Path) -> bool {
        self.watched()
            .filter_map(path_of)
            .any(|watched| watched.starts_with(directory))
    }

    /// Whether the script warned that it could not watch something.
    fn warned(&self) -> bool {
        self.lines()
            .any(|line| line.starts_with(b"cargo:warning=".as_slice()))
    }

    /// Whether the script ran again after `previous` was recorded.
    fn reran_since(&self, previous: &Self) -> bool {
        self.written != previous.written
    }
}

/// The path a directive names, read back from the bytes the script wrote.
///
/// A Unix pathname is a byte string, so the round trip is exact — which is what
/// lets a check assert that a name Git reports verbatim is watched verbatim.
#[cfg(unix)]
fn path_of(name: &[u8]) -> Option<PathBuf> {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};

    (!name.is_empty()).then(|| PathBuf::from(OsStr::from_bytes(name)))
}

/// The path a directive names, read back from the bytes the script wrote.
#[cfg(not(unix))]
fn path_of(name: &[u8]) -> Option<PathBuf> {
    (!name.is_empty())
        .then(|| std::str::from_utf8(name).ok().map(PathBuf::from))
        .flatten()
}

/// Host target triple, which Cargo also uses as a layout directory name.
fn host_triple() -> String {
    let program = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let output = Command::new(program)
        .arg("--verbose")
        .arg("--version")
        .output()
        .expect("rustc is runnable");
    let report = String::from_utf8(output.stdout).expect("rustc reports UTF-8");
    report
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_owned)
        .expect("rustc reports a host triple")
}

/// The ordinary case: an ignored output directory, clean tree, stable script.
///
/// This is the invariant every other check is a variation on. The output
/// directory is written by the build itself, so watching anything inside it
/// would make the script permanently dirty.
#[test]
fn an_ignored_output_directory_leaves_a_clean_tree_and_a_stable_script() {
    let workspace = Workspace::create("ignored-output");
    let target = workspace.path("target");

    let first = workspace.build(&target, &[]);
    assert_eq!(first.value("HYDRAX_SOURCE_STATE"), "clean");
    assert!(
        first.watches(&workspace.path("README.md")),
        "a tracked file outside any crate is watched"
    );
    assert!(
        !first.watches_inside(&target),
        "nothing inside the output directory is watched"
    );

    let second = workspace.build(&target, &[]);
    assert!(
        !second.reran_since(&first),
        "the script reran with nothing changed"
    );

    workspace.remove();
}

/// A nested output directory must not take its siblings out of `git status`.
///
/// Excluding the output directory's top-level path component was the first of
/// the four detection defects: with `CARGO_TARGET_DIR` under `crates/`, every
/// edit anywhere under `crates/` was reported as a clean tree.
#[test]
fn a_nested_output_directory_does_not_hide_an_edit_beside_it() {
    let workspace = Workspace::create("nested-output");
    let target = workspace.path("crates/hydrax-io/target");

    let first = workspace.build(&target, &[]);
    assert_eq!(first.value("HYDRAX_SOURCE_STATE"), "clean");

    workspace.write(
        "crates/hydrax-io/src/lib.rs",
        &format!("{LIBRARY}// An edit beside the output directory.\n"),
    );
    let second = workspace.build(&target, &[]);
    assert!(
        second.reran_since(&first),
        "an edit to a tracked file did not rerun the script"
    );
    assert_eq!(second.value("HYDRAX_SOURCE_STATE"), "modified");

    workspace.remove();
}

/// An output directory that already exists carries no `CACHEDIR.TAG`.
///
/// Cargo writes that marker only when it creates the directory itself, so the
/// second detection defect made the script rerun on every build for anyone
/// whose output directory pre-existed.
#[test]
fn a_precreated_output_directory_does_not_rerun_the_script() {
    let workspace = Workspace::create("precreated-output");
    let target = workspace.path("target");
    fs::create_dir_all(&target).expect("the output directory is creatable");

    let first = workspace.build(&target, &[]);
    assert!(
        !target.join("CACHEDIR.TAG").is_file(),
        "a pre-created output directory carries no marker"
    );
    assert_eq!(first.value("HYDRAX_SOURCE_STATE"), "clean");

    let second = workspace.build(&target, &[]);
    assert!(
        !second.reran_since(&first),
        "the script reran with nothing changed"
    );

    workspace.remove();
}

/// An output directory can carry no marker file at all.
///
/// `CARGO_CACHE_RUSTC_INFO=0` suppresses `.rustc_info.json`, which with a
/// pre-existing directory leaves nothing to recognise — the third detection
/// defect, and with a triple-named directory the fourth.
#[test]
fn a_markerless_output_directory_does_not_rerun_the_script() {
    let workspace = Workspace::create("markerless-output");
    let target = workspace.path("target");
    fs::create_dir_all(&target).expect("the output directory is creatable");
    let environment = [("CARGO_CACHE_RUSTC_INFO", "0")];

    let first = workspace.build(&target, &environment);
    assert!(
        !target.join("CACHEDIR.TAG").is_file() && !target.join(".rustc_info.json").is_file(),
        "a markerless output directory carries neither marker"
    );
    assert_eq!(first.value("HYDRAX_SOURCE_STATE"), "clean");

    let second = workspace.build(&target, &environment);
    assert!(
        !second.reran_since(&first),
        "the script reran with nothing changed"
    );

    workspace.remove();
}

/// A markerless output directory named after the target triple.
///
/// The last shape the layout fallback could not read: the directory Cargo
/// inserts for a cross-compiled build carries the triple as its name, so a
/// user-chosen output directory with the same name was mistaken for it and the
/// workspace root was proposed as the output root, which forced a rerun on
/// every build.
#[test]
fn a_triple_named_output_directory_does_not_rerun_the_script() {
    let workspace = Workspace::create("triple-named-output");
    let target = workspace.path(&host_triple());
    fs::create_dir_all(&target).expect("the output directory is creatable");
    let environment = [("CARGO_CACHE_RUSTC_INFO", "0")];

    let first = workspace.build(&target, &environment);
    assert_eq!(first.value("HYDRAX_SOURCE_STATE"), "clean");
    assert!(
        !first.watches_inside(&target),
        "nothing inside the output directory is watched"
    );
    assert!(
        first.watches(&workspace.path("Cargo.toml")),
        "the workspace manifest is watched"
    );

    let second = workspace.build(&target, &environment);
    assert!(
        !second.reran_since(&first),
        "the script reran with nothing changed"
    );

    workspace.remove();
}

/// An output directory holding tracked sources must not silence them.
///
/// The fourth detection defect: the candidate was excluded from `git status`
/// without checking what it held, so an edit to a source file inside it was
/// reported as a clean tree.
#[test]
fn an_output_directory_over_tracked_sources_does_not_hide_an_edit() {
    let workspace = Workspace::create("output-over-sources");
    let target = workspace.path("crates/hydrax-io");
    // The crate directory already exists, so Cargo writes no `CACHEDIR.TAG`
    // into it, and suppressing `.rustc_info.json` leaves only the ignored
    // profile directory behind. The tree is then dirty for one reason alone:
    // the edit below.
    let environment = [("CARGO_CACHE_RUSTC_INFO", "0")];

    let first = workspace.build(&target, &environment);
    assert_eq!(first.value("HYDRAX_SOURCE_STATE"), "clean");

    workspace.write(
        "crates/hydrax-io/src/lib.rs",
        &format!("{LIBRARY}// An edit inside the output directory.\n"),
    );
    let second = workspace.build(&target, &environment);
    assert!(
        second.reran_since(&first),
        "an edit to a tracked file did not rerun the script"
    );
    assert_eq!(second.value("HYDRAX_SOURCE_STATE"), "modified");

    workspace.remove();
}

/// An unignored output directory inside the repository is a dirty tree.
///
/// This is the deliberate trade of the tracked-file watch set: rather than
/// recognising Cargo's output and excluding it — the thing that went wrong
/// four times — `git status` runs unfiltered, and output the user chose to
/// leave unignored is reported for what it is. The script must still not rerun
/// on account of it.
#[test]
fn an_unignored_output_directory_reads_as_a_modified_tree() {
    let workspace = Workspace::create("unignored-output");
    let target = workspace.path("build-output");

    let first = workspace.build(&target, &[]);
    assert!(
        target.join("CACHEDIR.TAG").is_file(),
        "Cargo marked the output directory it created"
    );
    assert_eq!(first.value("HYDRAX_SOURCE_STATE"), "modified");

    let second = workspace.build(&target, &[]);
    assert!(
        !second.reran_since(&first),
        "the script reran with nothing changed"
    );

    workspace.remove();
}

/// A new commit has to reach the recorded revision.
///
/// The revision comes from `HEAD`, and committing on a branch rewrites only
/// that branch's ref file, so the ref is watched alongside `HEAD` itself.
#[test]
fn a_new_commit_reruns_the_script_and_changes_the_revision() {
    let workspace = Workspace::create("new-commit");
    let target = workspace.path("target");

    let first = workspace.build(&target, &[]);
    let revision = first.value("HYDRAX_GIT_REVISION").to_owned();
    assert!(
        matches!(revision.len(), 40 | 64),
        "a full object name was recorded, not {revision}"
    );

    workspace.commit("Add a second commit");
    let second = workspace.build(&target, &[]);
    assert!(
        second.reran_since(&first),
        "a commit did not rerun the script"
    );
    assert_ne!(
        second.value("HYDRAX_GIT_REVISION"),
        revision,
        "the recorded revision did not follow the commit"
    );
    assert_eq!(second.value("HYDRAX_SOURCE_STATE"), "clean");

    workspace.remove();
}

/// Deleting a tracked file is seen once, and does not strand the script dirty.
///
/// Cargo treats a watched path it cannot stat as permanently changed, so a file
/// that is gone must drop out of the watch set. The build before the deletion
/// was still watching it, which is what makes the deletion visible at all.
#[test]
fn a_deleted_tracked_file_reruns_the_script_exactly_once() {
    let workspace = Workspace::create("deleted-file");
    let target = workspace.path("target");
    let readme = workspace.path("README.md");

    let first = workspace.build(&target, &[]);
    assert!(
        first.watches(&readme),
        "the tracked file starts out watched"
    );

    fs::remove_file(&readme).expect("the tracked file is removable");
    let second = workspace.build(&target, &[]);
    assert!(
        second.reran_since(&first),
        "the deletion did not rerun the script"
    );
    assert_eq!(second.value("HYDRAX_SOURCE_STATE"), "modified");
    assert!(
        !second.watches(&readme),
        "a path Cargo cannot stat was left in the watch set"
    );

    let third = workspace.build(&target, &[]);
    assert!(
        !third.reran_since(&second),
        "the script stayed dirty after the deletion"
    );

    workspace.remove();
}

/// One pathname that is not valid UTF-8 does not empty the rest of the watch set.
///
/// The tracked listing is a stream of byte strings. Decoding it as text fails
/// on the first such name, and falling back to the crate's own sources over one
/// filename would leave every other tracked file able to change the reported
/// state unnoticed. The awkward file itself cannot be named in a directive —
/// Cargo decodes directives as UTF-8 and drops what it cannot decode, measured
/// directly — so the directory holding it is watched instead.
#[cfg(unix)]
#[test]
fn a_non_utf8_pathname_costs_only_itself() {
    let workspace = Workspace::create("non-utf8-name");
    let awkward = workspace.write_bytes(b"docs/\xffname.md", "A name that is not text.\n");
    workspace.track_all("Add a file whose name is not UTF-8");
    let target = workspace.path("target");

    let first = workspace.build(&target, &[]);
    assert_eq!(first.value("HYDRAX_SOURCE_STATE"), "clean");
    assert!(
        first.watches(&workspace.path("README.md")),
        "one awkward name emptied the rest of the watch set"
    );
    assert!(
        first.watches(&workspace.path("crates/hydrax-io/src/lib.rs")),
        "one awkward name emptied the rest of the watch set"
    );
    assert!(
        first.watches(&workspace.path("docs")),
        "the directory holding the awkward name is not watched"
    );
    assert!(!first.warned(), "nothing had to be given up");

    let second = workspace.build(&target, &[]);
    assert!(
        !second.reran_since(&first),
        "the script reran with nothing changed"
    );

    fs::write(&awkward, "Edited.\n").expect("the tracked file is writable");
    let third = workspace.build(&target, &[]);
    assert!(
        third.reran_since(&second),
        "an edit to the awkward name did not rerun the script"
    );
    assert_eq!(third.value("HYDRAX_SOURCE_STATE"), "modified");

    workspace.remove();
}

/// A pathname a directive cannot carry is covered by the directory holding it.
///
/// Two shapes cannot be carried: a line feed ends the directive, and Cargo
/// strips a carriage return from the end of a line, leaving a path it cannot
/// stat. Cargo walks a watched directory recursively, so watching the directory
/// keeps an edit to such a file visible — where discarding the name outright
/// left the file able to change the reported state in silence.
#[cfg(unix)]
#[test]
fn a_pathname_no_directive_can_carry_is_covered_by_its_directory() {
    let workspace = Workspace::create("unwritable-names");
    let broken = workspace.write_bytes(b"docs/line\nbreak.md", "A name holding a line feed.\n");
    let trailing = workspace.write_bytes(b"docs/trailing\r", "A name ending in a return.\n");
    workspace.track_all("Add files whose names a directive cannot carry");
    let target = workspace.path("target");
    let directory = workspace.path("docs");

    let first = workspace.build(&target, &[]);
    assert_eq!(first.value("HYDRAX_SOURCE_STATE"), "clean");
    assert!(
        !first.watches(&broken),
        "a line feed cannot be written into a directive"
    );
    assert!(
        !first.watches(&trailing),
        "Cargo strips a carriage return from the end of a line"
    );
    assert_eq!(
        first.watch_count(&directory),
        1,
        "the directory holding them is watched exactly once"
    );
    assert!(!first.warned(), "the directory stood in for both names");

    let second = workspace.build(&target, &[]);
    assert!(
        !second.reran_since(&first),
        "watching a directory left the script permanently dirty"
    );

    fs::write(&broken, "Edited.\n").expect("the tracked file is writable");
    let third = workspace.build(&target, &[]);
    assert!(
        third.reran_since(&second),
        "an edit to an unwritable name did not rerun the script"
    );
    assert_eq!(third.value("HYDRAX_SOURCE_STATE"), "modified");

    workspace.remove();
}

/// A carriage return anywhere but the end of a pathname needs no stand-in.
///
/// Cargo splits directives on line feeds and only trims a trailing return, so
/// such a name is watched as it is. Refusing every return would send a file to
/// a directory watch that does not need one.
#[cfg(unix)]
#[test]
fn a_carriage_return_inside_a_pathname_is_watched_directly() {
    let workspace = Workspace::create("interior-return");
    let interior = workspace.write_bytes(b"docs/car\rriage.md", "A name holding one inside.\n");
    workspace.track_all("Add a file whose name holds a carriage return");
    let target = workspace.path("target");

    let first = workspace.build(&target, &[]);
    assert!(
        first.watches(&interior),
        "the name was not watched as it is"
    );
    assert_eq!(first.value("HYDRAX_SOURCE_STATE"), "clean");

    let second = workspace.build(&target, &[]);
    assert!(
        !second.reran_since(&first),
        "the script reran with nothing changed"
    );

    fs::write(&interior, "Edited.\n").expect("the tracked file is writable");
    let third = workspace.build(&target, &[]);
    assert!(
        third.reran_since(&second),
        "an edit to the name did not rerun the script"
    );
    assert_eq!(third.value("HYDRAX_SOURCE_STATE"), "modified");

    workspace.remove();
}

/// A tracked file that cannot be watched at all is reported, not dropped.
///
/// With the output directory inside the repository, the only directory that
/// could stand in for a file at the repository root is the root itself, and
/// watching that would walk Cargo's output and rerun the script on every build.
/// The file is then left out — but said out loud, because the alternative is a
/// source state that quietly goes stale.
#[cfg(unix)]
#[test]
fn a_pathname_nothing_can_cover_is_reported_rather_than_dropped() {
    let workspace = Workspace::create("unwatchable-name");
    workspace.write_bytes(
        b"line\nbreak.md",
        "A name holding a line feed, at the root.\n",
    );
    workspace.track_all("Add an unwritable name at the repository root");
    let target = workspace.path("target");

    let first = workspace.build(&target, &[]);
    assert!(
        first.warned(),
        "a tracked file left out of the watch set was not reported"
    );
    assert!(
        first.watches(&workspace.path("README.md")),
        "the rest of the listing is still watched"
    );
    assert!(
        !first.watches_inside(&target),
        "nothing inside the output directory is watched"
    );
    assert_eq!(first.value("HYDRAX_SOURCE_STATE"), "clean");

    let second = workspace.build(&target, &[]);
    assert!(
        !second.reran_since(&first),
        "the script reran with nothing changed"
    );

    workspace.remove();
}
