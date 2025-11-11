/// Error message utilities - provides helpers for creating consistent, helpful error messages
/// following best practices: specific context, explanation of impact, and solution guidance
use crate::error::DotfilesError;
use std::path::Path;

/// Format error messages consistently
/// Pattern: [What happened] [Why it matters] [What to do] [Related elements]
#[allow(dead_code)]
pub struct ErrorBuilder {
    what: String,
    why: Option<String>,
    solution: Option<String>,
    context: Vec<(String, String)>,
}

#[allow(dead_code)]
impl ErrorBuilder {
    pub fn new(what: impl Into<String>) -> Self {
        Self {
            what: what.into(),
            why: None,
            solution: None,
            context: Vec::new(),
        }
    }

    pub fn why(mut self, why: impl Into<String>) -> Self {
        self.why = Some(why.into());
        self
    }

    pub fn solution(mut self, solution: impl Into<String>) -> Self {
        self.solution = Some(solution.into());
        self
    }

    pub fn context(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.context.push((key.into(), value.into()));
        self
    }

    pub fn build_config_error(self) -> DotfilesError {
        DotfilesError::Config(self.format())
    }

    pub fn build_path_error(self) -> DotfilesError {
        DotfilesError::Path(self.format())
    }

    pub fn build_io_error(self) -> DotfilesError {
        DotfilesError::Io(std::io::Error::other(self.format()))
    }

    fn format(&self) -> String {
        let mut msg = format!("What: {}", self.what);

        if let Some(ref why) = self.why {
            msg.push_str(&format!("\n  Why: {}", why));
        }

        for (key, value) in &self.context {
            msg.push_str(&format!("\n  {}: {}", key, value));
        }

        if let Some(ref solution) = self.solution {
            msg.push_str(&format!("\n  Solution: {}", solution));
        }

        msg
    }
}

/// Common error scenarios with pre-built helpers

/// File doesn't exist error with helpful context
#[allow(dead_code)]
pub fn file_not_found(path: &Path, context_type: &str) -> DotfilesError {
    DotfilesError::Path(format!(
        "What: {} file does not exist\n  \
         Path: {}\n  \
         Why: Required {} file is missing or at an unexpected location\n  \
         Solution:\n    \
         - Verify the file path is correct: `ls -la {}`\n    \
         - Check for typos in file names or directories\n    \
         - Use absolute paths if relative paths are ambiguous",
        context_type,
        path.display(),
        context_type,
        path.display()
    ))
}

/// Home directory not found error
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
pub fn invalid_path_computation(from_path: &Path, to_path: &Path, reason: &str) -> DotfilesError {
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

/// Invalid configuration value error
pub fn invalid_config_value(
    key: &str,
    invalid_value: &str,
    valid_options: &[&str],
    config_file: &str,
) -> DotfilesError {
    let valid_list = valid_options
        .iter()
        .map(|o| format!("  - {}", o))
        .collect::<Vec<_>>()
        .join("\n");

    DotfilesError::Config(format!(
        "What: Invalid configuration value\n  \
         Key: {}\n  \
         Invalid value: '{}'\n  \
         Why: This value is not recognized and will cause incorrect behavior\n  \
         Valid options:\n{}\n  \
         Config file: {}\n  \
         Solution:\n    \
         - Edit {} and change the value\n    \
         - Choose one of the valid options listed above\n    \
         - Run `flux validate` to verify the fix",
        key, invalid_value, valid_list, config_file, config_file
    ))
}

/// Symlink target validation error (security)
pub fn symlink_target_outside_repo(target: &Path, repo_path: &Path) -> DotfilesError {
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
pub fn dest_outside_home(dest: &Path, home: &Path) -> DotfilesError {
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

/// File operation error with context
#[allow(dead_code)]
pub fn file_operation_failed(operation: &str, file: &Path, reason: &str) -> DotfilesError {
    DotfilesError::Io(std::io::Error::other(format!(
        "What: File operation failed\n  \
         Operation: {}\n  \
         File: {}\n  \
         Reason: {}\n  \
         Solution:\n    \
         - Check file permissions: `ls -l {}`\n    \
         - Ensure the file is not locked by another process\n    \
         - Verify sufficient disk space is available\n    \
         - Try running with elevated privileges if needed",
        operation,
        file.display(),
        reason,
        file.display()
    )))
}

/// Profile not found error with suggestions
#[allow(dead_code)]
pub fn profile_not_found(profile_name: &str, available_profiles: &[String]) -> DotfilesError {
    let profile_list = if available_profiles.is_empty() {
        "  - default (always available)".to_string()
    } else {
        available_profiles
            .iter()
            .map(|p| format!("  - {}", p))
            .collect::<Vec<_>>()
            .join("\n")
    };

    DotfilesError::ProfileNotFound(format!(
        "What: Profile '{}' does not exist\n  \
         Why: The specified profile has not been created yet\n  \
         Available profiles:\n{}\n  \
         Solution:\n    \
         - Create the profile: `flux profile create {}`\n    \
         - Or switch to an existing profile: `flux profile switch <name>`\n    \
         - List all profiles: `flux profile list`",
        profile_name, profile_list, profile_name
    ))
}

/// Git operation error with troubleshooting
#[allow(dead_code)]
pub fn git_operation_failed(operation: &str, repo_path: &Path, reason: &str) -> DotfilesError {
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

/// Backup restore error
#[allow(dead_code)]
pub fn backup_restore_failed(backup_path: &Path, target: &Path, reason: &str) -> DotfilesError {
    DotfilesError::Path(format!(
        "What: Failed to restore file from backup\n  \
         Backup: {}\n  \
         Target: {}\n  \
         Reason: {}\n  \
         Solution:\n    \
         - Verify backup exists and is readable: `ls -la {}`\n    \
         - Check that target directory exists or can be created\n    \
         - Ensure you have write permissions to the target location\n    \
         - Run with elevated privileges if needed",
        backup_path.display(),
        target.display(),
        reason,
        backup_path.display()
    ))
}

// Macros for common patterns

/// Create a file not found error quickly
#[macro_export]
macro_rules! err_file_not_found {
    ($path:expr, $context:expr) => {
        $crate::error_utils::file_not_found($path, $context)
    };
}

/// Create a home directory not found error
#[macro_export]
macro_rules! err_home_dir_not_found {
    () => {
        $crate::error_utils::home_dir_not_found()
    };
}

/// Create a config file read error
#[macro_export]
macro_rules! err_config_file {
    ($msg:expr) => {
        $crate::error::DotfilesError::Config(format!(
            "What: Configuration error\n  \
             What: {}\n  \
             Why: The configuration file is invalid or unreadable\n  \
             Config location: ~/.config/flux/config.toml\n  \
             Solution:\n    \
             - Run `flux validate` to check for issues\n    \
             - Verify the TOML syntax is correct\n    \
             - Check file permissions: `ls -la ~/.config/flux/`",
            $msg
        ))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_builder_basic() {
        let err = ErrorBuilder::new("File not found")
            .why("Required configuration is missing")
            .solution("Check the file path")
            .context("Path", "/path/to/file")
            .build_path_error();

        let msg = err.to_string();
        assert!(msg.contains("What: File not found"));
        assert!(msg.contains("Why: Required configuration is missing"));
        assert!(msg.contains("Solution"));
        assert!(msg.contains("Path: /path/to/file"));
    }

    #[test]
    fn test_home_dir_not_found_message() {
        let err = home_dir_not_found();
        let msg = err.to_string();
        assert!(msg.contains("$HOME"));
        assert!(msg.contains("Solution"));
    }
}
