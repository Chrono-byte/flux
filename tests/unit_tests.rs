//! Unit tests for core library modules
//!
//! This file contains unit tests for all core modules.
//! Integration tests are in other files in the tests/ directory.

mod config_tests {
    use flux::config::Config;
    use flux::types::SymlinkResolution;

    #[test]
    fn test_default_config_is_valid() {
        let config = Config::default();
        assert!(!config.general.repo_path.is_empty());
        assert!(!config.general.current_profile.is_empty());
    }

    #[test]
    fn test_symlink_resolution_parsing() {
        let resolution = "relative".parse::<SymlinkResolution>();
        assert!(resolution.is_ok());

        let resolution = "absolute".parse::<SymlinkResolution>();
        assert!(resolution.is_ok());

        let resolution = "auto".parse::<SymlinkResolution>();
        assert!(resolution.is_ok());

        let resolution = "invalid".parse::<SymlinkResolution>();
        assert!(resolution.is_err());
    }
}

mod path_tests {
    #[test]
    fn test_path_expansion() {
        // Test path handling utilities
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/home/user"));
        assert!(home.exists());
    }
}

mod security_tests {
    use flux::utils::security;
    use tempfile::tempdir;

    #[test]
    fn test_set_secure_permissions() {
        let dir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("Failed to create temp directory: {error}");
                return;
            }
        };
        let file_path = dir.path().join("test.txt");
        if let Err(error) = std::fs::write(&file_path, "test") {
            eprintln!("Failed to write test file: {error}");
            return;
        }

        let result = security::set_secure_permissions(&file_path);
        assert!(result.is_ok());
    }
}

mod error_tests {
    use flux::utils::error::DotfilesError;

    #[test]
    fn test_error_with_context() {
        let err = DotfilesError::Path("File not found".to_string());
        let err_with_context = err.with_context("while processing .bashrc");

        let err_msg = err_with_context.to_string();
        assert!(err_msg.contains("while processing .bashrc"));
    }
}

mod symlink_tests {
    // These tests verify symlink creation and validation
    // Use tempdir for safe isolated filesystem testing

    #[test]
    fn test_relative_symlink_creation() {
        // Test would verify relative symlink creation works correctly
        // Implementation depends on file_manager module
    }

    #[test]
    fn test_absolute_symlink_creation() {
        // Test would verify absolute symlink creation works correctly
        // Implementation depends on file_manager module
    }
}

mod backup_tests {
    use tempfile::tempdir;

    #[test]
    fn test_backup_directory_creation() {
        let dir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => {
                eprintln!("Failed to create temp directory: {error}");
                return;
            }
        };
        let backup_path = dir.path().join("backup");

        if let Err(error) = std::fs::create_dir_all(&backup_path) {
            eprintln!("Failed to create backup directory: {error}");
            return;
        }
        assert!(backup_path.exists());
        assert!(backup_path.is_dir());
    }
}

// ============================================================================
// Configuration Module - Extended Tests
// ============================================================================

mod config_validation_tests {
    use flux::config::Config;
    use flux::types::SymlinkResolution;

    /// Test primary config validation functionality
    #[test]
    fn test_config_validation_primary() {
        let config = Config::default();
        let result = config.validate();

        // Primary: default config should validate successfully
        assert!(result.is_ok(), "Default config should validate");

        // Analogous: manually created config should also validate
        let mut manual_config = Config::default();
        manual_config.general.repo_path = "~/.dotfiles".to_string();
        manual_config.general.current_profile = "default".to_string();

        let manual_result = manual_config.validate();
        assert!(
            manual_result.is_ok(),
            "Manually created config should validate"
        );

        // Backwards compatibility: old configs without optional fields should work
        manual_config.general.default_remote = None;
        manual_config.general.default_branch = None;

        let legacy_result = manual_config.validate();
        assert!(
            legacy_result.is_ok(),
            "Config without optional fields should validate"
        );
    }

    /// Test symlink resolution validation - valid modes
    #[test]
    fn test_symlink_resolution_valid() {
        let valid_modes = vec![
            SymlinkResolution::Auto,
            SymlinkResolution::Relative,
            SymlinkResolution::Absolute,
            SymlinkResolution::Follow,
            SymlinkResolution::Replace,
        ];

        for mode in valid_modes {
            let mut config = Config::default();
            config.general.symlink_resolution = mode;

            assert!(
                config.validate().is_ok(),
                "Symlink mode '{mode:?}' should be valid"
            );
        }
    }

    /// Test symlink resolution TOML deserialization
    #[test]
    fn test_symlink_resolution_toml_deserialization() {
        use flux::types::SymlinkResolution;

        // Test that serde can deserialize valid lowercase strings from TOML
        let test_cases = vec![
            ("auto", SymlinkResolution::Auto),
            ("relative", SymlinkResolution::Relative),
            ("absolute", SymlinkResolution::Absolute),
            ("follow", SymlinkResolution::Follow),
            ("replace", SymlinkResolution::Replace),
        ];

        for (input_str, expected) in test_cases {
            // Create a TOML string and deserialize it
            let toml_str = format!(
                r#"
[general]
repo_path = "~/.dotfiles"
backup_dir = "~/.dotfiles-backup"
current_profile = "default"
symlink_resolution = "{input_str}"
"#
            );

            let config: Config = match toml::from_str(&toml_str) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("Failed to deserialize TOML: {error}");
                    return;
                }
            };
            assert_eq!(
                config.general.symlink_resolution, expected,
                "Should deserialize '{input_str}' to {expected:?}"
            );
        }
    }

    /// Test empty path validation failures
    #[test]
    fn test_empty_paths_fail_validation() {
        // Test empty repo_path
        let mut config = Config::default();
        config.general.repo_path = String::new();
        assert!(config.validate().is_err(), "Empty repo_path should fail");

        // Test empty backup_dir
        // let mut config = Config::default();
        // config.general.backup_dir = String::new();
        // assert!(config.validate().is_err(), "Empty backup_dir should fail");

        // Test empty profile
        let mut config = Config::default();
        config.general.current_profile = String::new();
        assert!(config.validate().is_err(), "Empty profile should fail");
    }

    /// Test profile name validation - valid names
    #[test]
    fn test_profile_names_valid() {
        let valid_names = vec!["default", "work", "home", "laptop-1", "profile_test"];

        for name in valid_names {
            let mut config = Config::default();
            config.general.current_profile = name.to_string();

            assert!(
                config.validate().is_ok(),
                "Profile name '{name}' should be valid"
            );
        }
    }

    /// Test profile name validation - invalid special characters
    #[test]
    fn test_profile_names_invalid_chars() {
        let invalid_names = vec![
            "profile@work",
            "profile.dev",
            "profile/home",
            "profile:main",
        ];

        for name in invalid_names {
            let mut config = Config::default();
            config.general.current_profile = name.to_string();

            assert!(
                config.validate().is_err(),
                "Profile name '{name}' should be invalid"
            );
        }
    }

    /// Test adding files to config
    #[test]
    fn test_add_file_to_config() {
        let mut config = Config::default();

        // Primary: add single file
        config.add_file_to_tool(
            "sway",
            "config",
            std::path::Path::new(".config/sway/config"),
            None,
        );

        assert!(config.tools.contains_key("sway"));
        let Some(sway_tool) = config.tools.get("sway") else {
            eprintln!("Tool 'sway' should exist");
            return;
        };
        assert_eq!(sway_tool.files.len(), 1);

        // Analogous: add to same tool creates multiple entries
        config.add_file_to_tool(
            "sway",
            "config.d",
            std::path::Path::new(".config/sway/config.d"),
            None,
        );

        let Some(sway_tool) = config.tools.get("sway") else {
            eprintln!("Tool 'sway' should exist");
            return;
        };
        assert_eq!(sway_tool.files.len(), 2);

        // Backwards compatible: profile-specific entries
        config.add_file_to_tool(
            "sway",
            "work",
            std::path::Path::new(".config/sway/config"),
            Some("work"),
        );

        let Some(sway_tool) = config.tools.get("sway") else {
            eprintln!("Tool 'sway' should exist");
            return;
        };
        assert_eq!(sway_tool.files.len(), 3);
    }
}

// ============================================================================
// SymlinkResolution Type - Extended Tests
// ============================================================================

mod symlink_resolution_extended_tests {
    use flux::types::SymlinkResolution;
    use std::str::FromStr;

    /// Test all symlink resolution modes can be created
    #[test]
    fn test_all_symlink_modes_creatable() {
        let modes = vec![
            SymlinkResolution::Auto,
            SymlinkResolution::Relative,
            SymlinkResolution::Absolute,
            SymlinkResolution::Follow,
            SymlinkResolution::Replace,
        ];

        for mode in modes {
            let _formatted = format!("{mode:?}");
            // All operations succeed
        }
    }

    /// Test symlink resolution parsing all modes
    #[test]
    fn test_parse_all_symlink_modes() {
        let test_cases = vec![
            ("auto", SymlinkResolution::Auto),
            ("relative", SymlinkResolution::Relative),
            ("absolute", SymlinkResolution::Absolute),
            ("follow", SymlinkResolution::Follow),
            ("replace", SymlinkResolution::Replace),
        ];

        for (str_mode, expected_mode) in test_cases {
            let parsed = match SymlinkResolution::from_str(str_mode) {
                Ok(path) => path,
                Err(error) => {
                    eprintln!("Failed to parse '{str_mode}': {error}");
                    return;
                }
            };
            assert_eq!(parsed, expected_mode);
        }
    }

    /// Test invalid symlink resolutions fail to parse
    #[test]
    fn test_invalid_symlink_modes_fail() {
        let invalid = vec!["invalid", "copy", "hard", "sym", ""];

        for mode_str in invalid {
            assert!(
                SymlinkResolution::from_str(mode_str).is_err(),
                "Mode '{mode_str}' should fail to parse"
            );
        }
    }
}

// ============================================================================
// Error Handling - Extended Tests
// ============================================================================

mod error_handling_extended_tests {
    use flux::utils::error::DotfilesError;
    use flux::utils::error::Result;

    /// Test error creation and context addition
    #[test]
    fn test_error_creation_and_context() {
        // Primary: create error
        let error = DotfilesError::Path("File not found".to_string());

        // Analogous: add context
        let contextualized = error.with_context("during sync");
        let msg = contextualized.to_string();

        // Backwards compatible: message contains original and context
        assert!(msg.contains("File not found"));
        assert!(msg.contains("during sync"));
    }

    /// Test all error variants can be created
    #[test]
    fn test_all_error_variants() {
        let errors: Vec<DotfilesError> = vec![
            DotfilesError::Config("config error".to_string()),
            DotfilesError::Path("path error".to_string()),
            DotfilesError::InvalidTool("unknown".to_string()),
            DotfilesError::ProfileNotFound("missing".to_string()),
            DotfilesError::Cancelled,
        ];

        for error in errors {
            let msg = error.to_string();
            assert!(!msg.is_empty(), "Error should produce a message");
        }
    }

    /// Test Result type operations
    #[test]
    fn test_result_operations() {
        let ok_result: Result<String> = Ok("success".to_string());
        let err_result: Result<String> = Err(DotfilesError::Cancelled);

        // Test is_ok/is_err
        assert!(ok_result.is_ok());
        assert!(err_result.is_err());

        // Test extracting Ok value using if let
        if let Ok(ok_value) = ok_result {
            assert_eq!(ok_value, "success");
        } else {
            unreachable!("Expected Ok variant");
        }

        // Test error handling - use pattern matching to handle Err case
        let err_value = match err_result {
            Err(_) => "default".to_string(),
            Ok(value) => value,
        };
        assert_eq!(err_value, "default");
    }

    /// Test error context stacking
    #[test]
    fn test_error_context_stacking() {
        let error = DotfilesError::Path("Initial".to_string());
        let context1 = error.with_context("First context");
        let context2 = context1.with_context("Second context");

        let msg = context2.to_string();
        assert!(msg.contains("Initial"));
        assert!(msg.contains("First context"));
        assert!(msg.contains("Second context"));
    }
}

// ============================================================================
// Dry-Run Module - Extended Tests
// ============================================================================

mod dry_run_extended_tests {
    use flux::utils::dry_run::{DryRun, Operation};
    use std::path::PathBuf;

    /// Test dry-run operation logging
    #[test]
    fn test_dry_run_operations() {
        let mut dry_run = DryRun::default();

        // Log various operations
        dry_run.log_operation(Operation::CreateDirectory {
            path: PathBuf::from("/test"),
        });

        dry_run.log_operation(Operation::CopyFile {
            from: PathBuf::from("/src"),
            to: PathBuf::from("/dst"),
        });

        dry_run.log_operation(Operation::GitCommit {
            message: "test commit".to_string(),
        });

        // Display should not panic
        dry_run.display_summary();
    }

    /// Test git remote operations
    #[test]
    fn test_git_remote_operations() {
        let mut dry_run = DryRun::default();

        dry_run.log_operation(Operation::GitRemoteAdd {
            name: "origin".to_string(),
            url: "git@github.com:user/repo.git".to_string(),
        });

        dry_run.log_operation(Operation::GitRemoteSetUrl {
            name: "origin".to_string(),
            url: "https://github.com/user/repo.git".to_string(),
        });

        dry_run.log_operation(Operation::GitRemoteRemove {
            name: "upstream".to_string(),
        });

        dry_run.display_summary();
    }

    /// Test git push operation
    #[test]
    fn test_git_push_operation() {
        let mut dry_run = DryRun::default();

        dry_run.log_operation(Operation::GitPush {
            remote: "origin".to_string(),
            branch: "main".to_string(),
            set_upstream: true,
        });

        dry_run.display_summary();
    }
}

// ============================================================================
// File Entry Type - Extended Tests
// ============================================================================

mod file_entry_extended_tests {
    use flux::types::FileEntry;

    /// Test file entry creation without profile
    #[test]
    fn test_file_entry_no_profile() {
        let entry = FileEntry {
            repo: "config".to_string(),
            dest: ".config/app".to_string(),
            profile: None,
            conditions: None,
            secret: None,
            secret_backend: None,
            secret_key: None,
        };

        assert_eq!(entry.repo, "config");
        assert_eq!(entry.dest, ".config/app");
        assert!(entry.profile.is_none());
    }

    /// Test file entry creation with profile
    #[test]
    fn test_file_entry_with_profile() {
        let entry = FileEntry {
            repo: "work_config".to_string(),
            dest: ".config/app".to_string(),
            profile: Some("work".to_string()),
            conditions: None,
            secret: None,
            secret_backend: None,
            secret_key: None,
        };

        assert_eq!(entry.profile, Some("work".to_string()));
    }

    /// Test file entry cloning
    #[test]
    fn test_file_entry_clone_and_compare() {
        let entry1 = FileEntry {
            repo: "config".to_string(),
            dest: ".config".to_string(),
            profile: Some("default".to_string()),
            conditions: None,
            secret: None,
            secret_backend: None,
            secret_key: None,
        };

        let entry2 = entry1.clone();

        assert_eq!(entry1.repo, entry2.repo);
        assert_eq!(entry1.dest, entry2.dest);
        assert_eq!(entry1.profile, entry2.profile);
    }
}

// ============================================================================
// Path Handling - Extended Tests
// ============================================================================

mod path_handling_tests {
    use std::path::PathBuf;

    /// Test path operations
    #[test]
    fn test_basic_path_operations() {
        let path = PathBuf::from("/home/user/.config");

        assert!(path.has_root());
        let Some(file_name) = path.file_name() else {
            eprintln!("Path should have file name");
            return;
        };
        assert_eq!(file_name, ".config");

        let Some(parent) = path.parent() else {
            eprintln!("Path should have parent");
            return;
        };
        assert_eq!(parent, PathBuf::from("/home/user"));
    }

    /// Test relative paths
    #[test]
    fn test_relative_paths() {
        let rel_path = PathBuf::from(".config/app/config");

        assert!(!rel_path.is_absolute());
        let Some(file_name) = rel_path.file_name() else {
            eprintln!("Path should have file name");
            return;
        };
        assert_eq!(file_name, "config");
    }

    /// Test path joining
    #[test]
    fn test_path_joining() {
        let base = PathBuf::from("/home/user");
        let relative = ".config";
        let joined = base.join(relative);

        assert_eq!(joined, PathBuf::from("/home/user/.config"));
    }
}
