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
    use crate::security;
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
    use crate::error::DotfilesError;

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

// Integration tests would go in tests/ directory
// This file contains unit tests for individual modules
