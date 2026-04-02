//! flux - A dotfiles manager with symlink-based sync
//!
//! This is the binary entry point for the Flux dotfiles manager.
//! The library functionality is provided in the `flux-lib` crate.

mod cli;

use clap::Parser;
use cli::{Cli, run};
use colored::{Colorize, control};
use flux_lib::config::{Config, EnvironmentConfig};
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

    let env_config = match EnvironmentConfig::load() {
        Ok(config) => {
            if !cli.quiet {
                config.display_summary();
            }
            config
        }
        Err(err) => {
            eprintln!(
                "{} Failed to load environment configuration: {}",
                "Error:".red().bold(),
                err
            );
            eprintln!(
                "{} Please check your environment variables",
                "Help:".yellow().bold()
            );
            std::process::exit(1);
        }
    };

    logging::init_logging(cli.log_level.as_deref());

    if let Err(err) = run(cli, env_config) {
        eprintln!("{} {}", "Error:".red().bold(), err);
        std::process::exit(1);
    }
}
