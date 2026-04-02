//! CLI router for the Flux dotfiles manager.

use super::args::{Cli, Commands};
use super::commands::{
    handle_add_command, handle_apply_command, handle_check_command, handle_commit_command,
    handle_completion_command, handle_config_command, handle_doctor_command,
    handle_history_command, handle_init_command, handle_ls_files_command, handle_maintain_command,
    handle_profile_command, handle_pull_command, handle_push_command, handle_remote_command,
    handle_rm_command, handle_status_command,
};
use flux_lib::config::EnvironmentConfig;
use flux_lib::utils::Result;

/// Main command router.
///
/// # Parameters
///
/// * `cli`: The parsed CLI arguments.
/// * `_env_config`: The environment configuration (validated at startup).
///
/// # Returns
///
/// Returns the result of the command execution.
///
/// # Errors
///
/// Returns an error if any command fails.
pub(super) fn run(cli: Cli, _env_config: EnvironmentConfig) -> Result<()> {
    match cli.command {
        Commands::Init { repo_path } => handle_init_command(repo_path)?,
        Commands::Add {
            tool,
            file,
            dest,
            profile,
            dry_run,
            from_repo,
        } => handle_add_command(
            &tool,
            &file,
            dest.as_deref(),
            profile.as_deref(),
            dry_run,
            from_repo,
        )?,
        Commands::Commit {
            profile,
            message,
            dry_run,
            verbose,
        } => handle_commit_command(profile.as_deref(), message.as_deref(), dry_run, verbose)?,
        Commands::Rm {
            tool,
            file,
            dry_run,
        } => handle_rm_command(&tool, &file, dry_run)?,
        Commands::LsFiles { profile } => handle_ls_files_command(profile.as_deref())?,
        Commands::Apply {
            profile,
            dry_run,
            diff,
            yes,
            description,
            force,
        } => handle_apply_command(
            profile.as_deref(),
            dry_run,
            diff,
            yes,
            description.as_deref(),
            force,
        )?,
        Commands::History {
            limit,
            profile,
            kind,
            since,
            json,
        } => handle_history_command(
            limit,
            profile.as_deref(),
            kind.as_deref(),
            since.as_deref(),
            json,
        )?,
        Commands::Profile { command } => handle_profile_command(command)?,
        Commands::Config { command } => handle_config_command(&command)?,
        Commands::Remote { command } => handle_remote_command(command)?,
        Commands::Push {
            remote,
            branch,
            set_upstream,
            dry_run,
            timeout,
        } => tokio::runtime::Runtime::new()?.block_on(handle_push_command(
            remote,
            branch,
            set_upstream,
            dry_run,
            timeout,
        ))?,
        Commands::Pull {
            remote,
            branch,
            dry_run,
            timeout,
        } => tokio::runtime::Runtime::new()?
            .block_on(handle_pull_command(remote, branch, dry_run, timeout))?,
        Commands::Status {
            profile,
            conflicts,
            json,
        } => handle_status_command(profile.as_deref(), conflicts, json)?,
        Commands::Maintain { command } => return handle_maintain_command(command),
        Commands::Check { profile } => handle_check_command(profile.as_deref(), cli.quiet)?,
        Commands::Doctor => handle_doctor_command()?,
        Commands::Completion { shell } => handle_completion_command(&shell),
    }

    Ok(())
}
