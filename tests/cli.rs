//! Integration tests for the Hydrax command-line interface.

use assert_cmd::Command;
use predicates::prelude::predicate;

#[test]
fn cli_reports_package_name_in_help() {
    let mut command = Command::cargo_bin("hydrax").expect("binary exists");

    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Core-collapse supernova"));
}

#[test]
fn cli_reports_version() {
    let mut command = Command::cargo_bin("hydrax").expect("binary exists");

    command
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("hydrax 0.1.0"));
}
