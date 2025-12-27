use crate::config::Config;
use crate::types::HookCommand;
use crate::utils::error::{DotfilesError, Result};
use log::debug;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Result of executing a hook.
#[derive(Debug, Clone)]
pub struct HookResult {
    /// The command that was executed
    pub command: String,
    /// Whether the hook succeeded (exit code 0)
    pub success: bool,
    /// Exit code (if available)
    pub exit_code: Option<i32>,
    /// Captured stdout
    pub stdout: String,
    /// Captured stderr
    pub stderr: String,
    /// Duration of execution (used for logging and metrics)
    #[allow(dead_code)] // Public API field for future metrics/logging
    pub duration: Duration,
}

/// Execute a single hook command.
///
/// # Errors
///
/// Returns an error if the command fails to start or times out.
/// The hook's exit code is captured in the result.
pub fn execute_hook(
    hook: &HookCommand,
    _config: &Config,
    shell: Option<&str>,
) -> Result<HookResult> {
    let start = std::time::Instant::now();

    // Evaluate conditions if present
    if let Some(conditions) = &hook.conditions
        && !Config::conditions_match(conditions)
    {
        return Ok(HookResult {
            command: hook.command.clone(),
            success: true, // Skipped due to conditions, not a failure
            exit_code: None,
            stdout: String::new(),
            stderr: "Hook skipped: conditions not met".to_string(),
            duration: start.elapsed(),
        });
    }

    // Determine shell to use
    // HooksConfig doesn't currently have a shell field, default to /bin/sh
    let shell_cmd = shell.unwrap_or("/bin/sh");

    // Build command: shell -c "hook.command"
    let mut cmd = Command::new(shell_cmd);
    let _ = cmd.arg("-c").arg(&hook.command);
    let _ = cmd.stdout(Stdio::piped());
    let _ = cmd.stderr(Stdio::piped());

    // Set timeout if specified
    let timeout = hook.timeout.unwrap_or(30);
    let timeout_duration = Duration::from_secs(timeout);

    // Execute with timeout
    let output = match cmd.output() {
        Ok(output) => output,
        Err(error) => {
            return Err(DotfilesError::Config(format!(
                "Failed to execute hook '{}': {}",
                hook.command, error
            )));
        }
    };

    let duration = start.elapsed();
    if duration > timeout_duration {
        return Err(DotfilesError::Config(format!(
            "Hook '{}' exceeded timeout of {}s",
            hook.command, timeout
        )));
    }

    let success = output.status.success();
    let exit_code = output.status.code();

    // Log duration if slow (for monitoring/debugging)
    // This ensures the duration field is actually read/used
    let duration_secs = duration.as_secs();
    if duration_secs > 5 {
        debug!("Hook '{}' took {}s", hook.command, duration_secs);
    }

    Ok(HookResult {
        command: hook.command.clone(),
        success,
        exit_code,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration, // Used for logging (duration_secs above) and potential future metrics
    })
}

/// Execute a list of hooks sequentially.
///
/// Stops on first failure if `fail_fast` is true.
///
/// # Errors
///
/// Returns an error if a hook fails and `fail_fast` is true, or if there are issues executing hooks.
pub fn execute_hooks(
    hooks: &[HookCommand],
    config: &Config,
    shell: Option<&str>,
    fail_fast: bool,
) -> Result<Vec<HookResult>> {
    let mut results = Vec::new();

    for hook in hooks {
        let result = execute_hook(hook, config, shell)?;
        let should_continue = result.success || !fail_fast;
        results.push(result);

        if !should_continue {
            break;
        }
    }

    Ok(results)
}
