//! Command-line entry point for Hydrax.

use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use hydrax::{
    EnergyDiagnostics, FiniteDifference, Observer, OutputRecord, RunRecord, ThirdDerivativeStencil,
    Waveform, Window, kiloparsecs_to_m, radiated_energy, read_configuration, sinusoidal_quadrupole,
    waveform_from_quadrupole, write_provenance,
};
use serde::{Deserialize, Serialize};

/// Core-collapse supernova gravitational-wave simulation tools.
#[derive(Debug, Parser)]
// The `about` text is the doc comment above, not this crate's package
// description: the binary presents itself as Hydrax rather than as its
// packaging crate.
#[command(name = "hydrax", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate an analytic sinusoidal quadrupole waveform as CSV.
    Demo(DemoArgs),
}

#[derive(Debug, Args)]
struct DemoArgs {
    /// Read run settings from this TOML configuration file.
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,
    #[command(flatten)]
    settings: DemoSettings,
}

/// One run's settings as supplied by a caller.
///
/// Every setting is optional at this stage: a setting left off the command line
/// falls through to the configuration file named by `--config`, and a setting
/// absent from both falls through to the default documented in `--help`. The
/// keys of a configuration file are the field names below.
#[derive(Debug, Default, Args, Deserialize)]
#[serde(deny_unknown_fields)]
struct DemoSettings {
    /// Source duration in seconds [default: 0.02].
    #[arg(long)]
    duration_s: Option<f64>,
    /// Source sampling rate in hertz [default: 16384].
    #[arg(long)]
    sample_rate_hz: Option<f64>,
    /// Quadrupole oscillation frequency in hertz [default: 800].
    #[arg(long)]
    frequency_hz: Option<f64>,
    /// Quadrupole amplitude in kg m² [default: 1e38].
    #[arg(long)]
    amplitude_kg_m2: Option<f64>,
    /// Source distance in kiloparsecs [default: 10].
    #[arg(long)]
    distance_kpc: Option<f64>,
    /// Observer inclination from the positive z-axis in degrees [default: 0].
    #[arg(long)]
    inclination_deg: Option<f64>,
    /// Observer azimuth from the positive x-axis in degrees [default: 0].
    #[arg(long)]
    azimuth_deg: Option<f64>,
    /// Centered finite-difference stencil [default: five-point].
    #[arg(long, value_enum)]
    difference: Option<DifferenceArg>,
    /// Select time-domain, frequency-domain, or energy-summary CSV output
    /// [default: time].
    #[arg(long, value_enum)]
    output: Option<OutputArg>,
    /// Window used for frequency-domain output [default: hann].
    #[arg(long, value_enum)]
    window: Option<WindowArg>,
    /// Taper fraction when `--window tukey` is selected [default: 0.5].
    #[arg(long)]
    tukey_alpha: Option<f64>,
    /// Write a versioned JSON provenance sidecar to this path.
    #[arg(long, value_name = "PATH")]
    provenance: Option<PathBuf>,
}

impl DemoSettings {
    /// Keep every setting this value carries and take the rest from `fallback`.
    fn overriding(self, fallback: Self) -> Self {
        Self {
            duration_s: self.duration_s.or(fallback.duration_s),
            sample_rate_hz: self.sample_rate_hz.or(fallback.sample_rate_hz),
            frequency_hz: self.frequency_hz.or(fallback.frequency_hz),
            amplitude_kg_m2: self.amplitude_kg_m2.or(fallback.amplitude_kg_m2),
            distance_kpc: self.distance_kpc.or(fallback.distance_kpc),
            inclination_deg: self.inclination_deg.or(fallback.inclination_deg),
            azimuth_deg: self.azimuth_deg.or(fallback.azimuth_deg),
            difference: self.difference.or(fallback.difference),
            output: self.output.or(fallback.output),
            window: self.window.or(fallback.window),
            tukey_alpha: self.tukey_alpha.or(fallback.tukey_alpha),
            provenance: self.provenance.or(fallback.provenance),
        }
    }

    /// Take every remaining setting from the documented defaults.
    fn resolve(self) -> DemoConfiguration {
        let defaults = DemoConfiguration::default();
        DemoConfiguration {
            duration_s: self.duration_s.unwrap_or(defaults.duration_s),
            sample_rate_hz: self.sample_rate_hz.unwrap_or(defaults.sample_rate_hz),
            frequency_hz: self.frequency_hz.unwrap_or(defaults.frequency_hz),
            amplitude_kg_m2: self.amplitude_kg_m2.unwrap_or(defaults.amplitude_kg_m2),
            distance_kpc: self.distance_kpc.unwrap_or(defaults.distance_kpc),
            inclination_deg: self.inclination_deg.unwrap_or(defaults.inclination_deg),
            azimuth_deg: self.azimuth_deg.unwrap_or(defaults.azimuth_deg),
            difference: self.difference.unwrap_or(defaults.difference),
            output: self.output.unwrap_or(defaults.output),
            window: self.window.unwrap_or(defaults.window),
            tukey_alpha: self.tukey_alpha.unwrap_or(defaults.tukey_alpha),
            provenance: self.provenance.or(defaults.provenance),
        }
    }
}

/// One run's complete settings, with every default made explicit.
///
/// This is the value recorded in a provenance sidecar, so its field names are
/// both the recorded keys and the accepted configuration-file keys.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct DemoConfiguration {
    duration_s: f64,
    sample_rate_hz: f64,
    frequency_hz: f64,
    amplitude_kg_m2: f64,
    distance_kpc: f64,
    inclination_deg: f64,
    azimuth_deg: f64,
    difference: DifferenceArg,
    output: OutputArg,
    window: WindowArg,
    tukey_alpha: f64,
    provenance: Option<PathBuf>,
}

// The documented defaults live here, in one place, rather than in a Clap
// attribute per field: a configuration file has to fall through to the same
// values as an omitted flag.
impl Default for DemoConfiguration {
    fn default() -> Self {
        Self {
            duration_s: 0.02,
            sample_rate_hz: 16_384.0,
            frequency_hz: 800.0,
            amplitude_kg_m2: 1.0e38,
            distance_kpc: 10.0,
            inclination_deg: 0.0,
            azimuth_deg: 0.0,
            difference: DifferenceArg::FivePoint,
            output: OutputArg::Time,
            window: WindowArg::Hann,
            tukey_alpha: 0.5,
            provenance: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum DifferenceArg {
    ThreePoint,
    FivePoint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum OutputArg {
    Time,
    Spectrum,
    Energy,
}

impl OutputArg {
    /// Return the stable schema identifier for this CSV output.
    const fn schema(self) -> &'static str {
        match self {
            Self::Time => "hydrax.waveform.time.csv.v1",
            Self::Spectrum => "hydrax.waveform.spectrum.csv.v1",
            Self::Energy => "hydrax.waveform.energy.csv.v1",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum WindowArg {
    Rectangular,
    Hann,
    Tukey,
}

impl From<DifferenceArg> for FiniteDifference {
    fn from(value: DifferenceArg) -> Self {
        match value {
            DifferenceArg::ThreePoint => Self::ThreePoint,
            DifferenceArg::FivePoint => Self::FivePoint,
        }
    }
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("error: {error}");
        std::process::exit(2);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match cli.command {
        Command::Demo(arguments) => write_demo(arguments),
    }
}

fn write_demo(arguments: DemoArgs) -> Result<(), Box<dyn std::error::Error>> {
    let file = match &arguments.config {
        Some(path) => read_configuration(path)?,
        None => DemoSettings::default(),
    };
    let configuration = arguments.settings.overriding(file).resolve();
    validate_positive("duration", configuration.duration_s)?;
    validate_positive("sample rate", configuration.sample_rate_hz)?;
    validate_positive("frequency", configuration.frequency_hz)?;
    validate_finite("amplitude", configuration.amplitude_kg_m2)?;
    validate_finite("distance", configuration.distance_kpc)?;
    validate_finite("inclination", configuration.inclination_deg)?;
    validate_finite("azimuth", configuration.azimuth_deg)?;
    validate_finite("Tukey alpha", configuration.tukey_alpha)?;
    if matches!(configuration.output, OutputArg::Time | OutputArg::Spectrum) {
        validate_positive("distance", configuration.distance_kpc)?;
    }

    let interval_count = (configuration.duration_s * configuration.sample_rate_hz).round();
    if !interval_count.is_finite() || interval_count > f64::from(u32::MAX - 1) {
        return Err("duration and sample rate request too many samples".into());
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sample_count = interval_count as usize + 1;
    let angular_frequency = 2.0 * std::f64::consts::PI * configuration.frequency_hz;
    let source = sinusoidal_quadrupole(
        configuration.amplitude_kg_m2,
        angular_frequency,
        0.0,
        1.0 / configuration.sample_rate_hz,
        sample_count,
    )?;
    match configuration.output {
        OutputArg::Energy => {
            let diagnostics = radiated_energy(&source, ThirdDerivativeStencil::SevenPoint)?;
            write_energy(&diagnostics)?;
        }
        OutputArg::Time | OutputArg::Spectrum => {
            let observer = Observer::new(
                kiloparsecs_to_m(configuration.distance_kpc),
                configuration.inclination_deg.to_radians(),
                configuration.azimuth_deg.to_radians(),
            )?;
            let waveform =
                waveform_from_quadrupole(&source, observer, configuration.difference.into())?;
            match configuration.output {
                OutputArg::Time => write_time_domain(&waveform)?,
                OutputArg::Spectrum => write_spectrum(
                    &waveform,
                    match configuration.window {
                        WindowArg::Rectangular => Window::Rectangular,
                        WindowArg::Hann => Window::Hann,
                        WindowArg::Tukey => Window::Tukey {
                            alpha: configuration.tukey_alpha,
                        },
                    },
                )?,
                OutputArg::Energy => unreachable!("energy output handled above"),
            }
        }
    }
    if let Some(path) = &configuration.provenance {
        write_provenance(
            path,
            RunRecord {
                command: "demo",
                configuration: &configuration,
            },
            OutputRecord {
                media_type: "text/csv",
                schema: configuration.output.schema(),
            },
        )?;
    }
    Ok(())
}

fn write_time_domain(waveform: &Waveform) -> Result<(), Box<dyn std::error::Error>> {
    let mut output = BufWriter::new(std::io::stdout().lock());
    writeln!(output, "time_s,h_plus,h_cross")?;
    for (index, (h_plus, h_cross)) in waveform.h_plus().iter().zip(waveform.h_cross()).enumerate() {
        let time = waveform.time_s(index).ok_or("waveform index is invalid")?;
        writeln!(output, "{time:.16e},{h_plus:.16e},{h_cross:.16e}")?;
    }
    output.flush()?;
    Ok(())
}

fn write_spectrum(waveform: &Waveform, window: Window) -> Result<(), Box<dyn std::error::Error>> {
    let spectrum = waveform.spectrum(window)?;
    let mut output = BufWriter::new(std::io::stdout().lock());
    writeln!(
        output,
        "frequency_hz,h_plus_real_s,h_plus_imaginary_s,h_cross_real_s,h_cross_imaginary_s"
    )?;
    for (index, (h_plus, h_cross)) in spectrum.h_plus().iter().zip(spectrum.h_cross()).enumerate() {
        let frequency = spectrum
            .frequency_hz(index)
            .ok_or("spectrum index is invalid")?;
        writeln!(
            output,
            "{frequency:.16e},{:.16e},{:.16e},{:.16e},{:.16e}",
            h_plus.real_s(),
            h_plus.imaginary_s(),
            h_cross.real_s(),
            h_cross.imaginary_s()
        )?;
    }
    output.flush()?;
    Ok(())
}

fn write_energy(diagnostics: &EnergyDiagnostics) -> Result<(), Box<dyn std::error::Error>> {
    let end_time = diagnostics
        .time_s(diagnostics.power_w().len() - 1)
        .ok_or("energy diagnostic index is invalid")?;
    let mut output = BufWriter::new(std::io::stdout().lock());
    writeln!(
        output,
        "start_time_s,end_time_s,total_energy_j,average_power_w"
    )?;
    writeln!(
        output,
        "{:.16e},{end_time:.16e},{:.16e},{:.16e}",
        diagnostics.start_time_s(),
        diagnostics.total_energy_j(),
        diagnostics.average_power_w()
    )?;
    output.flush()?;
    Ok(())
}

fn validate_finite(name: &str, value: f64) -> Result<(), Box<dyn std::error::Error>> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(format!("{name} must be finite, got {value}").into())
    }
}

fn validate_positive(name: &str, value: f64) -> Result<(), Box<dyn std::error::Error>> {
    validate_finite(name, value)?;
    if value > 0.0 {
        Ok(())
    } else {
        Err(format!("{name} must be positive, got {value}").into())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::{CommandFactory, ValueEnum};
    use hydrax::read_configuration;

    use super::{Cli, DemoConfiguration, DemoSettings, DifferenceArg, OutputArg, WindowArg};

    /// A configuration whose every setting differs from the default.
    fn distinctive_configuration() -> DemoConfiguration {
        DemoConfiguration {
            duration_s: 0.05,
            sample_rate_hz: 8192.0,
            frequency_hz: 300.0,
            amplitude_kg_m2: 2.0e37,
            distance_kpc: 8.5,
            inclination_deg: 30.0,
            azimuth_deg: 45.0,
            difference: DifferenceArg::ThreePoint,
            output: OutputArg::Spectrum,
            window: WindowArg::Tukey,
            tukey_alpha: 0.25,
            provenance: Some(PathBuf::from("distinctive.provenance.json")),
        }
    }

    /// A configuration that shares no setting with the one above, so that a
    /// setting taken from the wrong source cannot go unnoticed.
    fn competing_configuration() -> DemoConfiguration {
        DemoConfiguration {
            duration_s: 0.07,
            sample_rate_hz: 4096.0,
            frequency_hz: 900.0,
            amplitude_kg_m2: 3.0e36,
            distance_kpc: 2.5,
            inclination_deg: 60.0,
            azimuth_deg: 15.0,
            difference: DifferenceArg::FivePoint,
            output: OutputArg::Energy,
            window: WindowArg::Rectangular,
            tukey_alpha: 0.75,
            provenance: Some(PathBuf::from("competing.provenance.json")),
        }
    }

    /// Express a resolved configuration as the settings that produce it.
    fn settings_of(configuration: &DemoConfiguration) -> DemoSettings {
        DemoSettings {
            duration_s: Some(configuration.duration_s),
            sample_rate_hz: Some(configuration.sample_rate_hz),
            frequency_hz: Some(configuration.frequency_hz),
            amplitude_kg_m2: Some(configuration.amplitude_kg_m2),
            distance_kpc: Some(configuration.distance_kpc),
            inclination_deg: Some(configuration.inclination_deg),
            azimuth_deg: Some(configuration.azimuth_deg),
            difference: Some(configuration.difference),
            output: Some(configuration.output),
            window: Some(configuration.window),
            tukey_alpha: Some(configuration.tukey_alpha),
            provenance: configuration.provenance.clone(),
        }
    }

    /// Render the long help of the `demo` subcommand.
    ///
    /// The width is set wide enough that no help line wraps, so a documented
    /// default stays on one line where it can be read back.
    fn demo_help() -> String {
        let mut command = Cli::command();
        let demo = command
            .find_subcommand_mut("demo")
            .expect("the demo subcommand exists");
        demo.clone().term_width(200).render_long_help().to_string()
    }

    /// Read the default documented for one option in the help text.
    fn documented_default(help: &str, flag: &str) -> String {
        let after_flag = help
            .split_once(flag)
            .unwrap_or_else(|| panic!("{flag} appears in the help text"))
            .1;
        let opening = "[default: ";
        let start = after_flag
            .find(opening)
            .unwrap_or_else(|| panic!("{flag} documents a default"))
            + opening.len();
        let length = after_flag[start..]
            .find(']')
            .unwrap_or_else(|| panic!("the default documented for {flag} is closed"));
        after_flag[start..start + length].to_owned()
    }

    #[test]
    fn nothing_supplied_resolves_to_the_defaults() {
        assert_eq!(
            DemoSettings::default().resolve(),
            DemoConfiguration::default()
        );
    }

    #[test]
    fn a_configuration_file_supplies_every_setting() {
        let expected = distinctive_configuration();

        let resolved = DemoSettings::default()
            .overriding(settings_of(&expected))
            .resolve();

        assert_eq!(resolved, expected);
    }

    #[test]
    fn the_command_line_supplies_every_setting() {
        let expected = distinctive_configuration();

        let resolved = settings_of(&expected)
            .overriding(DemoSettings::default())
            .resolve();

        assert_eq!(resolved, expected);
    }

    #[test]
    fn the_command_line_overrides_every_configured_setting() {
        let expected = distinctive_configuration();

        let resolved = settings_of(&expected)
            .overriding(settings_of(&competing_configuration()))
            .resolve();

        assert_eq!(resolved, expected);
    }

    #[test]
    fn an_omitted_flag_falls_through_to_the_configuration_file() {
        let file = competing_configuration();
        let command_line = DemoSettings {
            frequency_hz: Some(1234.0),
            ..DemoSettings::default()
        };

        let resolved = command_line.overriding(settings_of(&file)).resolve();

        assert_eq!(
            resolved,
            DemoConfiguration {
                frequency_hz: 1234.0,
                ..file
            }
        );
    }

    #[test]
    fn a_recorded_configuration_is_a_configuration_file() {
        // The provenance record and the configuration file share one key set,
        // so a recorded run can be replayed by handing the record back.
        let expected = distinctive_configuration();
        let directory = std::env::temp_dir().join(format!(
            "hydrax-cli-configuration-{}-recorded",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).expect("temporary directory is creatable");
        let path = directory.join("run.toml");
        std::fs::write(
            &path,
            toml::to_string(&expected).expect("a configuration is serializable as TOML"),
        )
        .expect("configuration file is writable");

        let reloaded: DemoSettings =
            read_configuration(&path).expect("a recorded configuration is a valid file");

        assert_eq!(reloaded.resolve(), expected);

        std::fs::remove_dir_all(&directory).expect("temporary directory is removable");
    }

    #[test]
    fn a_recorded_default_configuration_keeps_its_unset_sidecar_path() {
        // The default configuration records `provenance` as JSON null, which
        // must read back as an unset setting rather than as an error.
        let expected = DemoConfiguration::default();
        let record = serde_json::to_value(&expected).expect("a configuration is serializable");

        let reloaded: DemoSettings =
            serde_json::from_value(record).expect("a recorded configuration is accepted");

        assert_eq!(reloaded.resolve(), expected);
    }

    #[test]
    fn the_documented_defaults_are_the_resolved_defaults() {
        let help = demo_help();
        let defaults = DemoConfiguration::default();

        for (flag, expected) in [
            ("--duration-s", defaults.duration_s),
            ("--sample-rate-hz", defaults.sample_rate_hz),
            ("--frequency-hz", defaults.frequency_hz),
            ("--amplitude-kg-m2", defaults.amplitude_kg_m2),
            ("--distance-kpc", defaults.distance_kpc),
            ("--inclination-deg", defaults.inclination_deg),
            ("--azimuth-deg", defaults.azimuth_deg),
            ("--tukey-alpha", defaults.tukey_alpha),
        ] {
            let documented = documented_default(&help, flag);
            let parsed: f64 = documented
                .parse()
                .unwrap_or_else(|_| panic!("{flag} documents {documented} as a number"));
            assert!(
                (parsed - expected).abs() <= f64::EPSILON * expected.abs(),
                "{flag} documents {documented} but resolves to {expected}"
            );
        }

        assert_eq!(
            DifferenceArg::from_str(&documented_default(&help, "--difference"), false)
                .expect("the documented stencil is accepted"),
            defaults.difference
        );
        assert_eq!(
            OutputArg::from_str(&documented_default(&help, "--output"), false)
                .expect("the documented output is accepted"),
            defaults.output
        );
        assert_eq!(
            WindowArg::from_str(&documented_default(&help, "--window"), false)
                .expect("the documented window is accepted"),
            defaults.window
        );
    }
}
