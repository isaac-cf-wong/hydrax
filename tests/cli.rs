//! Integration tests for the Hydrax command-line interface.

use assert_cmd::Command;
use predicates::prelude::predicate;
use serde_json::Value;

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

#[test]
fn demo_rejects_non_finite_inactive_tukey_fraction() {
    let mut command = Command::cargo_bin("hydrax").expect("binary exists");

    command
        .args(["demo", "--output", "time", "--tukey-alpha", "NaN"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Tukey alpha must be finite"));
}

#[test]
fn demo_rejects_non_finite_inactive_distance() {
    let mut command = Command::cargo_bin("hydrax").expect("binary exists");

    command
        .args(["demo", "--output", "energy", "--distance-kpc", "NaN"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("distance must be finite"));
}

#[test]
fn demo_emits_energy_csv() {
    let mut command = Command::cargo_bin("hydrax").expect("binary exists");

    command
        .args(["demo", "--output", "energy"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with(
            "start_time_s,end_time_s,total_energy_j,average_power_w",
        ));
}

#[test]
fn demo_writes_versioned_provenance() {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock follows the Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "hydrax-provenance-{}-{}.json",
        std::process::id(),
        unique
    ));
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
            "--difference",
            "three-point",
            "--provenance",
        ])
        .arg(&path)
        .assert()
        .success();

    let contents = std::fs::read_to_string(&path).expect("provenance file exists");
    let document: Value = serde_json::from_str(&contents).expect("valid provenance JSON");
    assert_eq!(document["schema"], "hydrax.provenance.v1");
    assert_eq!(document["software"]["name"], "hydrax");
    assert_eq!(document["run"]["configuration"]["frequency_hz"], 500.0);
    assert_eq!(
        document["run"]["configuration"]["difference"],
        "three-point"
    );
    assert_eq!(document["output"]["schema"], "hydrax.waveform.time.csv.v1");
    assert!(
        document["build"]["dependencies"]
            .as_array()
            .expect("dependency array")
            .iter()
            .any(|dependency| dependency["name"] == "clap")
    );

    std::fs::remove_file(path).expect("temporary provenance can be removed");
}
