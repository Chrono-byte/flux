//! flux - A dotfiles manager with symlink-based sync
//!
//! This is the binary entry point for the Flux dotfiles manager.
//! The library functionality is provided in the `flux-lib` crate.

mod cli;

use clap::Parser;
use cli::Cli;
use colored::{Colorize, control};
use flux_lib::config::Config;
use flux_lib::utils::logging;
use std::path::Path;

fn main() {
    let cli = Cli::parse();

    if cli.no_color {
        control::set_override(false);
    }

    if let Some(config_path) = &cli.config {
        Config::set_path_override(Path::new(config_path));
    }

    logging::init_logging(cli.log_level.as_deref());

    if let Err(err) = { cli.command.execute() } {
        eprintln!("{} {}", "Error:".red().bold(), err);
        std::process::exit(1);
    }
}
