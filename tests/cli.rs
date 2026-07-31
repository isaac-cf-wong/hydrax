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

#[test]
fn demo_emits_waveform_csv() {
    let mut command = Command::cargo_bin("hydrax").expect("binary exists");

    command
        .args([
            "demo",
            "--duration-s",
            "0.001",
            "--sample-rate-hz",
            "10000",
            "--frequency-hz",
            "500",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("time_s,h_plus,h_cross\n"));
}

#[test]
fn demo_rejects_non_positive_distance() {
    let mut command = Command::cargo_bin("hydrax").expect("binary exists");

    command
        .args(["demo", "--distance-kpc", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("distance must be positive"));
}

#[test]
fn demo_emits_spectrum_csv() {
    let mut command = Command::cargo_bin("hydrax").expect("binary exists");

    command
        .args([
            "demo",
            "--duration-s",
            "0.01",
            "--sample-rate-hz",
            "10000",
            "--frequency-hz",
            "500",
            "--output",
            "spectrum",
            "--window",
            "rectangular",
        ])
        .assert()
        .success()
        .stdout(predicate::str::starts_with(
            "frequency_hz,h_plus_real_s,h_plus_imaginary_s",
        ));
}

#[test]
fn demo_rejects_invalid_tukey_fraction() {
    let mut command = Command::cargo_bin("hydrax").expect("binary exists");

    command
        .args([
            "demo",
            "--output",
            "spectrum",
            "--window",
            "tukey",
            "--tukey-alpha",
            "1.5",
        ])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Tukey alpha must be in [0, 1]"));
}
