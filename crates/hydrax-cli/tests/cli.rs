//! Integration tests for the Hydrax command-line interface.

use assert_cmd::Command;
use predicates::prelude::predicate;
use serde_json::Value;

/// Build a command for the `hydrax` binary under test.
fn hydrax_command() -> Command {
    Command::cargo_bin("hydrax").expect("binary exists")
}

#[test]
fn cli_reports_package_name_in_help() {
    let mut command = hydrax_command();

    command
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Core-collapse supernova"));
}

#[test]
fn cli_reports_version() {
    let mut command = hydrax_command();

    command
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(format!(
            "hydrax {}",
            env!("CARGO_PKG_VERSION")
        )));
}

#[test]
fn demo_emits_waveform_csv() {
    let mut command = hydrax_command();

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
    let mut command = hydrax_command();

    command
        .args(["demo", "--distance-kpc", "0"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("distance must be positive"));
}

#[test]
fn demo_emits_spectrum_csv() {
    let mut command = hydrax_command();

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
    let mut command = hydrax_command();

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
    let mut command = hydrax_command();

    command
        .args(["demo", "--output", "time", "--tukey-alpha", "NaN"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Tukey alpha must be finite"));
}

#[test]
fn demo_rejects_non_finite_inactive_distance() {
    let mut command = hydrax_command();

    command
        .args(["demo", "--output", "energy", "--distance-kpc", "NaN"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("distance must be finite"));
}

#[test]
fn demo_emits_energy_csv() {
    let mut command = hydrax_command();

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
    let mut command = hydrax_command();

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

/// Create a private directory for one test's files.
fn temporary_directory(name: &str) -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock follows the Unix epoch")
        .as_nanos();
    let directory =
        std::env::temp_dir().join(format!("hydrax-cli-{name}-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("temporary directory is creatable");
    directory
}

/// Write one configuration file and return its path.
fn write_configuration(directory: &std::path::Path, contents: &str) -> std::path::PathBuf {
    let path = directory.join("run.toml");
    std::fs::write(&path, contents).expect("configuration file is writable");
    path
}

#[test]
fn demo_takes_its_settings_from_a_configuration_file() {
    let directory = temporary_directory("configured");
    let sidecar = directory.join("run.provenance.json");
    let configuration = write_configuration(
        &directory,
        &format!(
            "duration_s = 0.001\n\
             sample_rate_hz = 10000.0\n\
             frequency_hz = 500.0\n\
             difference = \"three-point\"\n\
             output = \"spectrum\"\n\
             window = \"rectangular\"\n\
             provenance = {:?}\n",
            sidecar.to_str().expect("temporary path is UTF-8")
        ),
    );
    let mut command = hydrax_command();

    command
        .arg("demo")
        .arg("--config")
        .arg(&configuration)
        .assert()
        .success()
        .stdout(predicate::str::starts_with(
            "frequency_hz,h_plus_real_s,h_plus_imaginary_s",
        ));

    let contents = std::fs::read_to_string(&sidecar).expect("the configured sidecar exists");
    let document: Value = serde_json::from_str(&contents).expect("valid provenance JSON");
    let recorded = &document["run"]["configuration"];
    assert_eq!(recorded["frequency_hz"], 500.0);
    assert_eq!(recorded["sample_rate_hz"], 10000.0);
    assert_eq!(recorded["difference"], "three-point");
    assert_eq!(recorded["window"], "rectangular");
    // A setting the file leaves out keeps its documented default.
    assert_eq!(recorded["distance_kpc"], 10.0);
    assert_eq!(
        document["output"]["schema"],
        "hydrax.waveform.spectrum.csv.v1"
    );

    std::fs::remove_dir_all(&directory).expect("temporary directory is removable");
}

#[test]
fn a_command_line_flag_overrides_the_configuration_file() {
    let directory = temporary_directory("override");
    let sidecar = directory.join("run.provenance.json");
    let configuration = write_configuration(
        &directory,
        "duration_s = 0.001\nsample_rate_hz = 10000.0\nfrequency_hz = 500.0\n",
    );
    let mut command = hydrax_command();

    command
        .arg("demo")
        .arg("--config")
        .arg(&configuration)
        .args(["--frequency-hz", "700"])
        .arg("--provenance")
        .arg(&sidecar)
        .assert()
        .success();

    let contents = std::fs::read_to_string(&sidecar).expect("the sidecar exists");
    let document: Value = serde_json::from_str(&contents).expect("valid provenance JSON");
    let recorded = &document["run"]["configuration"];
    assert_eq!(recorded["frequency_hz"], 700.0);
    // The settings the flag did not name still come from the file.
    assert_eq!(recorded["sample_rate_hz"], 10000.0);

    std::fs::remove_dir_all(&directory).expect("temporary directory is removable");
}

#[test]
fn demo_reports_a_missing_configuration_file() {
    let directory = temporary_directory("absent");
    let mut command = hydrax_command();

    command
        .arg("demo")
        .arg("--config")
        .arg(directory.join("absent.toml"))
        .assert()
        .failure()
        .stderr(predicate::str::contains("cannot read run configuration"))
        .stderr(predicate::str::contains("absent.toml"));

    std::fs::remove_dir_all(&directory).expect("temporary directory is removable");
}

#[test]
fn demo_rejects_a_misspelled_configuration_key() {
    let directory = temporary_directory("misspelled");
    let configuration = write_configuration(&directory, "frequency_hertz = 500.0\n");
    let mut command = hydrax_command();

    command
        .arg("demo")
        .arg("--config")
        .arg(&configuration)
        .assert()
        .failure()
        .stderr(predicate::str::contains("frequency_hertz"))
        .stderr(predicate::str::contains("is invalid"));

    std::fs::remove_dir_all(&directory).expect("temporary directory is removable");
}

#[test]
fn demo_validates_a_configured_setting() {
    let directory = temporary_directory("invalid-distance");
    let configuration = write_configuration(&directory, "distance_kpc = 0.0\n");
    let mut command = hydrax_command();

    command
        .arg("demo")
        .arg("--config")
        .arg(&configuration)
        .assert()
        .failure()
        .stderr(predicate::str::contains("distance must be positive"));

    std::fs::remove_dir_all(&directory).expect("temporary directory is removable");
}

#[test]
fn demo_places_the_source_at_the_configured_distance_in_kiloparsecs() {
    // The strain scales as one over the source distance, so this pins the
    // kiloparsec-to-metre conversion the command line applies. Every constant
    // below is a published value written out here rather than read from Hydrax,
    // so agreement is with an external reference and not with the code.
    const GRAVITATIONAL_CONSTANT: f64 = 6.6743e-11;
    const SPEED_OF_LIGHT: f64 = 299_792_458.0;
    const TEN_KILOPARSEC_M: f64 = 3.0857e20;
    const AMPLITUDE_KG_M2: f64 = 1.0e38;
    const FREQUENCY_HZ: f64 = 100.0;

    let mut command = hydrax_command();
    let output = command
        .args([
            "demo",
            "--duration-s",
            "0.01",
            "--sample-rate-hz",
            "16384",
            "--frequency-hz",
            "100",
            "--amplitude-kg-m2",
            "1e38",
            "--distance-kpc",
            "10",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(output).expect("CSV output is UTF-8");
    let mut rows = text.lines();
    assert_eq!(rows.next(), Some("time_s,h_plus,h_cross"));
    let first: Vec<&str> = rows
        .next()
        .expect("the waveform has at least one sample")
        .split(',')
        .collect();
    let time_s: f64 = first[0].parse().expect("the time column is a number");
    let h_plus: f64 = first[1].parse().expect("the strain column is a number");

    // Q_xx = A cos(wt) and Q_yy = -A cos(wt) seen face on give
    // h_plus = -2 G A w^2 cos(wt) / (c^4 D).
    let angular_frequency = 2.0 * std::f64::consts::PI * FREQUENCY_HZ;
    let expected = -2.0
        * GRAVITATIONAL_CONSTANT
        * AMPLITUDE_KG_M2
        * angular_frequency.powi(2)
        * (angular_frequency * time_s).cos()
        / (SPEED_OF_LIGHT.powi(4) * TEN_KILOPARSEC_M);

    assert!(
        (h_plus - expected).abs() <= 1.0e-4 * expected.abs(),
        "expected a strain of {expected} at ten kiloparsecs, got {h_plus}"
    );
}
