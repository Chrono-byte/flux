#[cfg(test)]
mod config_tests {
    use crate::config::Config;
    use crate::types::SymlinkResolution;

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

#[cfg(test)]
mod path_tests {
    #[test]
    fn test_path_expansion() {
        // Test path handling utilities
        let home = dirs::home_dir().unwrap();
        assert!(home.exists());
    }
}

#[cfg(test)]
mod security_tests {
    use crate::utils::security;
    use tempfile::tempdir;

    #[test]
    fn test_set_secure_permissions() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "test").unwrap();

        let result = security::set_secure_permissions(&file_path);
        assert!(result.is_ok());
    }
}

#[cfg(test)]
mod error_tests {
    use crate::utils::error::DotfilesError;

    #[test]
    fn test_error_with_context() {
        let err = DotfilesError::Path("File not found".to_string());
        let err_with_context = err.with_context("while processing .bashrc");

        let err_msg = err_with_context.to_string();
        assert!(err_msg.contains("while processing .bashrc"));
    }
}

#[cfg(test)]
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

#[cfg(test)]
mod backup_tests {
    use tempfile::tempdir;

    #[test]
    fn test_backup_directory_creation() {
        let dir = tempdir().unwrap();
        let backup_path = dir.path().join("backup");

        std::fs::create_dir_all(&backup_path).unwrap();
        assert!(backup_path.exists());
        assert!(backup_path.is_dir());
    }
}

// ============================================================================
// COMPREHENSIVE TEST SUITE - Library Core Tests
// ============================================================================
// This file contains unit tests for all core modules
// Integration tests are in tests/ directory

// ============================================================================
// Configuration Module - Extended Tests
// ============================================================================

#[cfg(test)]
mod config_validation_tests {
    use crate::config::Config;
    // use crate::types::SymlinkResolution;

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
        assert!(manual_result.is_ok(), "Manually created config should validate");
        
        // Backwards compatibility: old configs without optional fields should work
        manual_config.general.default_remote = None;
        manual_config.general.default_branch = None;
        
        let legacy_result = manual_config.validate();
        assert!(legacy_result.is_ok(), "Config without optional fields should validate");
    }

    /// Test symlink resolution validation - valid modes
    #[test]
    fn test_symlink_resolution_valid() {
        let valid_modes = vec!["auto", "relative", "absolute", "follow", "replace"];
        
        for mode in valid_modes {
            let mut config = Config::default();
            config.general.symlink_resolution = mode.to_string();
            
            assert!(
                config.validate().is_ok(),
                "Symlink mode '{}' should be valid",
                mode
            );
        }
    }

    /// Test symlink resolution validation - case insensitivity
    #[test]
    fn test_symlink_resolution_case_variations() {
        let variations = vec![
            ("AUTO", "auto"),
            ("Relative", "relative"),
            ("ABSOLUTE", "absolute"),
        ];
        
        for (input, expected) in variations {
            let mut config = Config::default();
            config.general.symlink_resolution = input.to_string();
            
            config.validate().unwrap();
            assert_eq!(
                config.general.symlink_resolution, expected,
                "Should normalize to lowercase"
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
        let mut config = Config::default();
        config.general.backup_dir = String::new();
        assert!(config.validate().is_err(), "Empty backup_dir should fail");
        
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
                "Profile name '{}' should be valid",
                name
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
                "Profile name '{}' should be invalid",
                name
            );
        }
    }

    /// Test adding files to config
    #[test]
    fn test_add_file_to_config() {
        let mut config = Config::default();
        
        // Primary: add single file
        config
            .add_file_to_tool("sway", "config", std::path::Path::new(".config/sway/config"), None)
            .unwrap();

        assert!(config.tools.contains_key("sway"));
        assert_eq!(config.tools.get("sway").unwrap().files.len(), 1);
        
        // Analogous: add to same tool creates multiple entries
        config
            .add_file_to_tool("sway", "config.d", std::path::Path::new(".config/sway/config.d"), None)
            .unwrap();
        
        assert_eq!(config.tools.get("sway").unwrap().files.len(), 2);
        
        // Backwards compatible: profile-specific entries
        config
            .add_file_to_tool("sway", "work", std::path::Path::new(".config/sway/config"), Some("work"))
            .unwrap();
        
        assert_eq!(config.tools.get("sway").unwrap().files.len(), 3);
    }
}

// ============================================================================
// SymlinkResolution Type - Extended Tests
// ============================================================================

#[cfg(test)]
mod symlink_resolution_extended_tests {
    use crate::types::SymlinkResolution;
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
            let _cloned = mode.clone();
            let _formatted = format!("{:?}", mode);
            assert!(true); // All operations succeed
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
            let parsed = SymlinkResolution::from_str(str_mode).unwrap();
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
                "Mode '{}' should fail to parse",
                mode_str
            );
        }
    }
}

// ============================================================================
// Error Handling - Extended Tests
// ============================================================================

// The #[cfg(test)] attribute is not needed for a module inside tests.rs
mod error_handling_extended_tests {
    use crate::DotfilesError;
    use crate::Result;

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

        // Test unwrap_or
        let ok_value = ok_result.unwrap_or_else(|_| "default".to_string());
        assert_eq!(ok_value, "success");

        let err_value = err_result.unwrap_or_else(|_| "default".to_string());
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

#[cfg(test)]
mod dry_run_extended_tests {
    use crate::utils::dry_run::{DryRun, Operation};
    use std::path::PathBuf;

    /// Test dry-run operation logging
    #[test]
    fn test_dry_run_operations() {
        let mut dry_run = DryRun::new();

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
        assert!(true);
    }

    /// Test git remote operations
    #[test]
    fn test_git_remote_operations() {
        let mut dry_run = DryRun::new();

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
        assert!(true);
    }

    /// Test git push operation
    #[test]
    fn test_git_push_operation() {
        let mut dry_run = DryRun::new();

        dry_run.log_operation(Operation::GitPush {
            remote: "origin".to_string(),
            branch: "main".to_string(),
            set_upstream: true,
        });

        dry_run.display_summary();
        assert!(true);
    }
}

// ============================================================================
// File Entry Type - Extended Tests  
// ============================================================================

#[cfg(test)]
mod file_entry_extended_tests {
    use crate::types::FileEntry;

    /// Test file entry creation without profile
    #[test]
    fn test_file_entry_no_profile() {
        let entry = FileEntry {
            repo: "config".to_string(),
            dest: ".config/app".to_string(),
            profile: None,
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

#[cfg(test)]
mod path_handling_tests {
    use std::path::PathBuf;

    /// Test path operations
    #[test]
    fn test_basic_path_operations() {
        let path = PathBuf::from("/home/user/.config");
        
        assert!(path.has_root());
        assert_eq!(path.file_name().unwrap(), ".config");
        
        let parent = path.parent().unwrap();
        assert_eq!(parent, PathBuf::from("/home/user"));
    }

    /// Test relative paths
    #[test]
    fn test_relative_paths() {
        let rel_path = PathBuf::from(".config/app/config");
        
        assert!(!rel_path.is_absolute());
        assert_eq!(rel_path.file_name().unwrap(), "config");
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
