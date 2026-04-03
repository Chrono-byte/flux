/// Error message utilities - provides helpers for creating consistent, helpful error messages
/// following best practices: specific context, explanation of impact, and solution guidance
use crate::utils::error::DotfilesError;
use std::path::Path;

/// Home directory not found error
#[must_use]
pub fn home_dir_not_found() -> DotfilesError {
  DotfilesError::Config(
    "What: Could not determine home directory\n  \
         Why: The $HOME environment variable is not set or home directory lookup failed\n  \
         This is required for all dotfile operations\n  \
         Solution:\n    \
         - Check that $HOME is exported: `echo $HOME`\n    \
         - If empty, add to your shell config (~/.bashrc, ~/.zshrc, etc.):\n    \
           export HOME=\"/home/your_username\"\n    \
         - Restart your shell or run: `source ~/.bashrc`"
      .to_string(),
  )
}

/// Invalid path computation error
#[must_use]
pub fn invalid_path_computation(
  from_path: &Path,
  to_path: &Path,
  reason: &str,
) -> DotfilesError {
  DotfilesError::Path(format!(
    "What: Cannot compute relative path between two locations\n  \
         From: {}\n  \
         To: {}\n  \
         Why: {}\n  \
         This usually means the paths don't share a common ancestor,\n  \
         or one path is outside the expected directory structure\n  \
         Solution:\n    \
         - Verify both paths are correct\n    \
         - Use absolute paths for clarity\n    \
         - Check repository configuration in ~/.config/flux/config.toml",
    from_path.display(),
    to_path.display(),
    reason
  ))
}

/// Symlink target validation error (security)
#[must_use]
pub fn symlink_target_outside_repo(
  target: &Path,
  repo_path: &Path,
) -> DotfilesError {
  DotfilesError::Path(format!(
    "What: Security violation - symlink target is outside repository\n  \
         Target: {}\n  \
         Repository: {}\n  \
         Why: Symlink targets must be within the repository to prevent\n  \
         path traversal attacks and unintended file access\n  \
         Solution:\n    \
         - Verify the symlink points to a file in the repository\n    \
         - Check your file configuration in config.toml\n    \
         - Ensure the target path is correctly specified",
    target.display(),
    repo_path.display()
  ))
}

/// Destination path outside home error (security)
#[must_use]
pub fn dest_outside_home(
  dest: &Path,
  home: &Path,
) -> DotfilesError {
  DotfilesError::Path(format!(
    "What: Destination path is outside home directory\n  \
         Destination: {}\n  \
         Home directory: {}\n  \
         Why: All dotfile destinations must be within your home directory\n  \
         to keep your personal configuration contained\n  \
         Solution:\n    \
         - Verify the destination path is correct\n    \
         - Remove leading slashes if present\n    \
         - Check your config for absolute paths instead of relative ones",
    dest.display(),
    home.display()
  ))
}

/// Profile not found error with suggestions
#[must_use]
pub fn profile_not_found(
  profile_name: &str,
  available_profiles: &[String],
) -> DotfilesError {
  let profile_list = if available_profiles.is_empty() {
    "  - default (always available)".to_string()
  } else {
    available_profiles
      .iter()
      .map(|profile| format!("  - {profile}"))
      .collect::<Vec<_>>()
      .join("\n")
  };

  DotfilesError::ProfileNotFound(format!(
    "What: Profile '{profile_name}' does not exist\n  \
         Why: The specified profile has not been created yet\n  \
         Available profiles:\n{profile_list}\n  \
         Solution:\n    \
         - Create the profile: `flux profile create {profile_name}`\n    \
         - Or switch to an existing profile: `flux profile switch <name>`\n    \
         - List all profiles: `flux profile list`"
  ))
}

/// Git operation error with troubleshooting
#[must_use]
pub fn git_operation_failed(
  operation: &str,
  repo_path: &Path,
  reason: &str,
) -> DotfilesError {
  let error_msg = format!(
    "What: Git operation failed\n  \
         Operation: {}\n  \
         Repository: {}\n  \
         Reason: {}\n  \
         Solution:\n    \
         - Check repository status: `git -C {} status`\n    \
         - Check git configuration: `git -C {} config --list`\n    \
         - Verify remote is set: `git -C {} remote -v`\n    \
         - Check logs for more details: `git -C {} log --oneline -n 5`",
    operation,
    repo_path.display(),
    reason,
    repo_path.display(),
    repo_path.display(),
    repo_path.display(),
    repo_path.display()
  );
  // Use a generic IO error as a wrapper since git2::Error requires different construction
  DotfilesError::Io(std::io::Error::other(error_msg))
}
