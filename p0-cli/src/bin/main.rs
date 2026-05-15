use anyhow::Result;
use clap::Parser;
use p0_cli::Opts;

fn main() -> Result<()> {
    p0_cli::entry(Opts::parse())
}
