//! System detection utilities for condition evaluation

use std::env;
use std::path::Path;

/// Get the current operating system name
///
/// Returns standardized OS names: "linux", "macos", "windows", etc.
#[must_use]
pub fn get_os() -> String {
  env::consts::OS.to_string()
}

/// Get the current hostname
///
/// Returns the system hostname or "unknown" if unavailable
#[must_use]
pub fn get_hostname() -> String {
  hostname::get()
    .ok()
    .and_then(|name: std::ffi::OsString| name.into_string().ok())
    .unwrap_or_else(|| "unknown".to_string())
}

/// Get an environment variable value
///
/// Returns the value if set, None otherwise
#[must_use]
pub fn get_env_var(name: &str) -> Option<String> {
  env::var(name).ok()
}

/// Check if an environment variable is set (to any value)
#[must_use]
pub fn has_env_var(name: &str) -> bool {
  env::var(name).is_ok()
}

/// Check if a command exists in PATH
#[must_use]
pub fn command_exists(command: &str) -> bool {
  which::which(command).is_ok()
}

/// Check if a file or directory exists
#[must_use]
pub fn path_exists(path: &str) -> bool {
  // Expand tilde and environment variables
  let expanded = shellexpand::full(path).unwrap_or(std::borrow::Cow::Borrowed(path));
  Path::new(expanded.as_ref()).exists()
}

/// Get the current CPU architecture
///
/// Returns standardized arch names: "`x86_64`", "aarch64", etc.
#[must_use]
pub fn get_architecture() -> String {
  env::consts::ARCH.to_string()
}

/// Get the current shell name
///
/// Returns the shell name (e.g., "bash", "zsh", "fish") or "unknown"
#[must_use]
pub fn get_shell() -> String {
  env::var("SHELL")
    .ok()
    .and_then(|shell_path| {
      Path::new(&shell_path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(String::from)
    })
    .unwrap_or_else(|| "unknown".to_string())
}

/// Get the current username
///
/// Returns the username or "unknown" if unavailable
#[must_use]
pub fn get_username() -> String {
  env::var("USER")
    .or_else(|_| env::var("USERNAME"))
    .unwrap_or_else(|_| "unknown".to_string())
}

/// Check if the current user is in a specific group
///
/// Returns true if the user is in the group, false otherwise.
/// On non-Unix systems, this always returns false.
#[must_use]
pub fn user_in_group(groupname: &str) -> bool {
  #[cfg(unix)]
  {
    // Use the `groups` command as a safe alternative to libc calls
    use std::process::Command;

    let output = Command::new("groups").output();

    if let Ok(output) = output
      && output.status.success()
      && let Ok(groups_str) = String::from_utf8(output.stdout)
    {
      return groups_str
        .split_whitespace()
        .any(|group| group == groupname);
    }
    false
  }

  #[cfg(not(unix))]
  {
    // Group checking not supported on non-Unix systems
    let _ = groupname;
    false
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_get_os() {
    let os = get_os();
    assert!(!os.is_empty());
    // Should be one of the known OS values
    assert!(["linux", "macos", "windows", "freebsd", "openbsd", "netbsd"].contains(&os.as_str()));
  }

  #[test]
  fn test_get_hostname() {
    let hostname = get_hostname();
    assert!(!hostname.is_empty());
  }

  #[test]
  fn test_get_architecture() {
    let arch = get_architecture();
    assert!(!arch.is_empty());
    // Should be one of the known architectures
    assert!(["x86_64", "aarch64", "arm", "x86", "powerpc64", "riscv64"].contains(&arch.as_str()));
  }

  #[test]
  fn test_env_var() {
    // Test with PATH which should always exist
    assert!(has_env_var("PATH"));
    assert!(get_env_var("PATH").is_some());

    // Test with a variable that definitely doesn't exist
    assert!(!has_env_var("DEFINITELY_NOT_A_REAL_VAR_12345"));
    assert_eq!(get_env_var("DEFINITELY_NOT_A_REAL_VAR_12345"), None);
  }

  #[test]
  fn test_command_exists() {
    // Test with a command that should exist on all systems
    assert!(command_exists("ls") || command_exists("dir")); // ls on Unix, dir on Windows

    // Test with a command that definitely doesn't exist
    assert!(!command_exists("definitely_not_a_real_command_12345"));
  }

  #[test]
  fn test_path_exists() {
    // Test with current directory (should always exist)
    assert!(path_exists("."));

    // Test with a path that doesn't exist
    assert!(!path_exists("/path/that/definitely/does/not/exist/12345"));
  }

  #[test]
  fn test_get_shell() {
    let shell = get_shell();
    assert!(!shell.is_empty());
  }

  #[test]
  fn test_get_username() {
    let username = get_username();
    assert!(!username.is_empty());
    assert_ne!(username, "unknown");
  }
}
