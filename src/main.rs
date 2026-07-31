//! Command-line entry point for Hydrax.

use clap::Parser;

/// Core-collapse supernova gravitational-wave simulation tools.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();
}
