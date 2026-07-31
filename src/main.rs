//! Command-line entry point for Hydrax.

use std::io::{BufWriter, Write};

use clap::{Args, Parser, Subcommand, ValueEnum};
use hydrax::{
    FiniteDifference, Observer, PARSEC_M, Waveform, Window, sinusoidal_quadrupole,
    waveform_from_quadrupole,
};

/// Core-collapse supernova gravitational-wave simulation tools.
#[derive(Debug, Parser)]
#[command(version, about)]
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
    /// Select time-domain or frequency-domain CSV output.
    #[arg(long, value_enum, default_value_t = OutputArg::Time)]
    output: OutputArg,
    /// Window used for frequency-domain output.
    #[arg(long, value_enum, default_value_t = WindowArg::Hann)]
    window: WindowArg,
    /// Taper fraction when `--window tukey` is selected.
    #[arg(long, default_value_t = 0.5)]
    tukey_alpha: f64,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum DifferenceArg {
    ThreePoint,
    FivePoint,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputArg {
    Time,
    Spectrum,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
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
    validate_positive("distance", arguments.distance_kpc)?;
    validate_finite("amplitude", arguments.amplitude_kg_m2)?;
    validate_finite("inclination", arguments.inclination_deg)?;
    validate_finite("azimuth", arguments.azimuth_deg)?;

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
    let observer = Observer::new(
        arguments.distance_kpc * 1000.0 * PARSEC_M,
        arguments.inclination_deg.to_radians(),
        arguments.azimuth_deg.to_radians(),
    )?;
    let waveform = waveform_from_quadrupole(&source, observer, arguments.difference.into())?;

    match arguments.output {
        OutputArg::Time => write_time_domain(&waveform),
        OutputArg::Spectrum => write_spectrum(
            &waveform,
            match arguments.window {
                WindowArg::Rectangular => Window::Rectangular,
                WindowArg::Hann => Window::Hann,
                WindowArg::Tukey => Window::Tukey {
                    alpha: arguments.tukey_alpha,
                },
            },
        ),
    }
}

fn write_time_domain(waveform: &Waveform) -> Result<(), Box<dyn std::error::Error>> {
    let mut output = BufWriter::new(std::io::stdout().lock());
    writeln!(output, "time_s,h_plus,h_cross")?;
    for (index, (h_plus, h_cross)) in waveform.h_plus().iter().zip(waveform.h_cross()).enumerate() {
        let time = waveform.time_s(index).ok_or("waveform index is invalid")?;
        writeln!(output, "{time:.16e},{h_plus:.16e},{h_cross:.16e}")?;
    }
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
