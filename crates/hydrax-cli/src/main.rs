//! Command-line entry point for Hydrax.

use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use hydrax::{
    EnergyDiagnostics, FiniteDifference, Observer, OutputRecord, PARSEC_M, RunRecord,
    ThirdDerivativeStencil, Waveform, Window, radiated_energy, sinusoidal_quadrupole,
    waveform_from_quadrupole, write_provenance,
};
use serde::Serialize;

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

#[derive(Debug, Args, Serialize)]
struct DemoArgs {
    /// Source duration in seconds.
    #[arg(long, default_value_t = 0.02)]
    duration_s: f64,
    /// Source sampling rate in hertz.
    #[arg(long, default_value_t = 16_384.0)]
    sample_rate_hz: f64,
    /// Quadrupole oscillation frequency in hertz.
    #[arg(long, default_value_t = 800.0)]
    frequency_hz: f64,
    /// Quadrupole amplitude in kg m².
    #[arg(long, default_value_t = 1.0e38)]
    amplitude_kg_m2: f64,
    /// Source distance in kiloparsecs.
    #[arg(long, default_value_t = 10.0)]
    distance_kpc: f64,
    /// Observer inclination from the positive z-axis in degrees.
    #[arg(long, default_value_t = 0.0)]
    inclination_deg: f64,
    /// Observer azimuth from the positive x-axis in degrees.
    #[arg(long, default_value_t = 0.0)]
    azimuth_deg: f64,
    /// Centered finite-difference stencil.
    #[arg(long, value_enum, default_value_t = DifferenceArg::FivePoint)]
    difference: DifferenceArg,
    /// Select time-domain, frequency-domain, or energy-summary CSV output.
    #[arg(long, value_enum, default_value_t = OutputArg::Time)]
    output: OutputArg,
    /// Window used for frequency-domain output.
    #[arg(long, value_enum, default_value_t = WindowArg::Hann)]
    window: WindowArg,
    /// Taper fraction when `--window tukey` is selected.
    #[arg(long, default_value_t = 0.5)]
    tukey_alpha: f64,
    /// Write a versioned JSON provenance sidecar to this path.
    #[arg(long, value_name = "PATH")]
    provenance: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Serialize, ValueEnum)]
#[serde(rename_all = "kebab-case")]
enum DifferenceArg {
    ThreePoint,
    FivePoint,
}

#[derive(Debug, Clone, Copy, Serialize, ValueEnum)]
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

#[derive(Debug, Clone, Copy, Serialize, ValueEnum)]
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
        Command::Demo(arguments) => write_demo(&arguments),
    }
}

fn write_demo(arguments: &DemoArgs) -> Result<(), Box<dyn std::error::Error>> {
    validate_positive("duration", arguments.duration_s)?;
    validate_positive("sample rate", arguments.sample_rate_hz)?;
    validate_positive("frequency", arguments.frequency_hz)?;
    validate_finite("amplitude", arguments.amplitude_kg_m2)?;
    validate_finite("distance", arguments.distance_kpc)?;
    validate_finite("inclination", arguments.inclination_deg)?;
    validate_finite("azimuth", arguments.azimuth_deg)?;
    validate_finite("Tukey alpha", arguments.tukey_alpha)?;
    if matches!(arguments.output, OutputArg::Time | OutputArg::Spectrum) {
        validate_positive("distance", arguments.distance_kpc)?;
    }

    let interval_count = (arguments.duration_s * arguments.sample_rate_hz).round();
    if !interval_count.is_finite() || interval_count > f64::from(u32::MAX - 1) {
        return Err("duration and sample rate request too many samples".into());
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let sample_count = interval_count as usize + 1;
    let angular_frequency = 2.0 * std::f64::consts::PI * arguments.frequency_hz;
    let source = sinusoidal_quadrupole(
        arguments.amplitude_kg_m2,
        angular_frequency,
        0.0,
        1.0 / arguments.sample_rate_hz,
        sample_count,
    )?;
    match arguments.output {
        OutputArg::Energy => {
            let diagnostics = radiated_energy(&source, ThirdDerivativeStencil::SevenPoint)?;
            write_energy(&diagnostics)?;
        }
        OutputArg::Time | OutputArg::Spectrum => {
            let observer = Observer::new(
                arguments.distance_kpc * 1000.0 * PARSEC_M,
                arguments.inclination_deg.to_radians(),
                arguments.azimuth_deg.to_radians(),
            )?;
            let waveform =
                waveform_from_quadrupole(&source, observer, arguments.difference.into())?;
            match arguments.output {
                OutputArg::Time => write_time_domain(&waveform)?,
                OutputArg::Spectrum => write_spectrum(
                    &waveform,
                    match arguments.window {
                        WindowArg::Rectangular => Window::Rectangular,
                        WindowArg::Hann => Window::Hann,
                        WindowArg::Tukey => Window::Tukey {
                            alpha: arguments.tukey_alpha,
                        },
                    },
                )?,
                OutputArg::Energy => unreachable!("energy output handled above"),
            }
        }
    }
    if let Some(path) = &arguments.provenance {
        write_provenance(
            path,
            RunRecord {
                command: "demo",
                configuration: arguments,
            },
            OutputRecord {
                media_type: "text/csv",
                schema: arguments.output.schema(),
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
