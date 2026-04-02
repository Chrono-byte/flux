//! CLI module for the Flux dotfiles manager.
//!
//! This module provides the `Cli` struct and `run` function for executing the CLI.

use flux_lib::config::EnvironmentConfig;
use flux_lib::utils::Result;

pub mod args;
mod commands;
mod handler;

pub use args::Cli;

/// Run the CLI with the given arguments and environment configuration.
pub fn run(cli: Cli, env_config: EnvironmentConfig) -> Result<()> {
    handler::run(cli, env_config)
}
