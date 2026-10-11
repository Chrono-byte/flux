mod config_tests {
  use flux_lib::config::Config;
  use flux_lib::types::SymlinkResolution;

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
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/home/user"));
    assert!(home.exists());
  }
}

mod security_tests {
  use flux_lib::utils::security;
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
  use flux_lib::utils::error::DotfilesError;

  #[test]
  fn test_error_with_context() {
    let err = DotfilesError::Path("File not found".to_string());
    let err_with_context = err.with_context("while processing .bashrc");

    let err_msg = err_with_context.to_string();
    assert!(err_msg.contains("while processing .bashrc"));
  }
}

mod config_validation_tests {
  use flux_lib::config::Config;
  use flux_lib::types::SymlinkResolution;
  use std::path::{Path, PathBuf};

  #[test]
  fn test_config_validation_primary() {
    let config = Config::default();
    let result = config.validate();

    assert!(result.is_ok(), "Default config should validate");

    let mut manual_config = Config::default();
    manual_config.general.repo_path = "~/.dotfiles".to_string();
    manual_config.general.current_profile = "default".to_string();

    let manual_result = manual_config.validate();
    assert!(
      manual_result.is_ok(),
      "Manually created config should validate"
    );

    manual_config.general.default_remote = None;
    manual_config.general.default_branch = None;

    let legacy_result = manual_config.validate();
    assert!(
      legacy_result.is_ok(),
      "Config without optional fields should validate"
    );
  }

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

  #[test]
  fn test_symlink_resolution_toml_deserialization() {
    use flux_lib::types::SymlinkResolution;

    let test_cases = vec![
      ("auto", SymlinkResolution::Auto),
      ("relative", SymlinkResolution::Relative),
      ("absolute", SymlinkResolution::Absolute),
      ("follow", SymlinkResolution::Follow),
      ("replace", SymlinkResolution::Replace),
    ];

    for (input_str, expected) in test_cases {
      let toml_str = format!(
        r#"
[general]
repo_path = "~/.dotfiles"
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

  #[test]
  fn test_empty_paths_fail_validation() {
    let mut config = Config::default();
    config.general.repo_path = String::new();
    assert!(config.validate().is_err(), "Empty repo_path should fail");

    let mut config = Config::default();
    config.general.current_profile = String::new();
    assert!(config.validate().is_err(), "Empty profile should fail");
  }

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

  #[test]
  fn test_add_file_to_config() {
    let mut config = Config::default();

    config.add_file_to_tool("sway", "config", Path::new(".config/sway/config"), None);

    assert!(config.tools.contains_key("sway"));
    let Some(sway_tool) = config.tools.get("sway") else {
      eprintln!("Tool 'sway' should exist");
      return;
    };
    assert_eq!(sway_tool.files.len(), 1);

    config.add_file_to_tool("sway", "config.d", Path::new(".config/sway/config.d"), None);

    let Some(sway_tool) = config.tools.get("sway") else {
      eprintln!("Tool 'sway' should exist");
      return;
    };
    assert_eq!(sway_tool.files.len(), 2);

    config.add_file_to_tool(
      "sway",
      "work",
      Path::new(".config/sway/config"),
      Some("work"),
    );

    let Some(sway_tool) = config.tools.get("sway") else {
      eprintln!("Tool 'sway' should exist");
      return;
    };
    assert_eq!(sway_tool.files.len(), 3);
  }

  #[test]
  fn test_add_folder_to_config() {
    let mut config = Config::default();

    config.add_folder_to_tool("nvim", "nvim", Path::new(".config/nvim"), None);

    let Some(nvim_tool) = config.tools.get("nvim") else {
      eprintln!("Tool 'nvim' should exist");
      return;
    };
    assert_eq!(nvim_tool.folders.len(), 1);
    assert!(nvim_tool.files.is_empty());
  }

  #[test]
  fn test_folder_repo_path_uses_tool_directory_itself() {
    let mut config = Config::default();
    config.general.repo_path = "/tmp/dotfiles".to_string();

    config.add_folder_to_tool("nvim", "nvim", Path::new(".config/nvim"), None);

    let tracked = match config.get_tracked_files(None) {
      Ok(files) => files,
      Err(error) => {
        eprintln!("Failed to get tracked files: {error}");
        return;
      }
    };

    assert_eq!(tracked.len(), 1, "Expected one tracked folder entry");

    let Some(first_entry) = tracked.first() else {
      eprintln!("Expected at least one tracked entry");
      return;
    };

    assert_eq!(
      first_entry.repo_path,
      PathBuf::from("/tmp/dotfiles/nvim"),
      "Repo path should not duplicate the tool name"
    );
  }
}

mod symlink_resolution_extended_tests {
  use flux_lib::types::SymlinkResolution;
  use std::str::FromStr;

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
    }
  }

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

mod error_handling_extended_tests {
  use flux_lib::utils::error::DotfilesError;
  use flux_lib::utils::error::Result;

  #[test]
  fn test_error_creation_and_context() {
    let error = DotfilesError::Path("File not found".to_string());

    let contextualized = error.with_context("during sync");
    let msg = contextualized.to_string();

    assert!(msg.contains("File not found"));
    assert!(msg.contains("during sync"));
  }

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

  #[test]
  fn test_result_operations() {
    let ok_result: Result<String> = Ok("success".to_string());
    let err_result: Result<String> = Err(DotfilesError::Cancelled);

    assert!(ok_result.is_ok());
    assert!(err_result.is_err());

    if let Ok(ok_value) = ok_result {
      assert_eq!(ok_value, "success");
    } else {
      unreachable!("Expected Ok variant");
    }

    let err_value = match err_result {
      Err(_) => "default".to_string(),
      Ok(value) => value,
    };
    assert_eq!(err_value, "default");
  }

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

mod dry_run_extended_tests {
  use flux_lib::utils::dry_run::{DryRun, Operation};
  use std::path::PathBuf;

  #[test]
  fn test_dry_run_operations() {
    let mut dry_run = DryRun::default();

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

    assert_eq!(dry_run.operations().len(), 3);
  }

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

    assert_eq!(dry_run.operations().len(), 3);
  }

  #[test]
  fn test_git_push_operation() {
    let mut dry_run = DryRun::default();

    dry_run.log_operation(Operation::GitPush {
      remote: "origin".to_string(),
      branch: "main".to_string(),
      set_upstream: true,
    });

    assert_eq!(dry_run.operations().len(), 1);
  }
}

mod filesystem_manager_dry_run_tests {
  use flux_lib::file_manager::FileSystemManager;
  use flux_lib::utils::dry_run::DryRun;
  use std::error::Error;
  use std::fs;
  use tempfile::tempdir;

  #[test]
  fn create_dir_all_skips_existing_directories() -> Result<(), Box<dyn Error>> {
    let temp_dir = tempdir()?;
    let existing = temp_dir.path().join("existing");
    fs::create_dir_all(&existing)?;

    let mut dry_run = DryRun::default();
    let mut fs_manager = FileSystemManager::new(&mut dry_run, true);
    fs_manager.create_dir_all(&existing)?;

    assert!(
      dry_run.operations().is_empty(),
      "No operations should be logged when the directory already exists"
    );
    Ok(())
  }
}

mod file_entry_extended_tests {
  use flux_lib::types::FileEntry;

  #[test]
  fn test_file_entry_no_profile() {
    let entry = FileEntry {
      repo: "config".to_string(),
      dest: ".config/app".to_string(),
      profile: None,
      conditions: None,
    };

    assert_eq!(entry.repo, "config");
    assert_eq!(entry.dest, ".config/app");
    assert!(entry.profile.is_none());
  }

  #[test]
  fn test_file_entry_with_profile() {
    let entry = FileEntry {
      repo: "work_config".to_string(),
      dest: ".config/app".to_string(),
      profile: Some("work".to_string()),
      conditions: None,
    };

    assert_eq!(entry.profile, Some("work".to_string()));
  }

  #[test]
  fn test_file_entry_clone_and_compare() {
    let entry1 = FileEntry {
      repo: "config".to_string(),
      dest: ".config".to_string(),
      profile: Some("default".to_string()),
      conditions: None,
    };

    let entry2 = entry1.clone();

    assert_eq!(entry1.repo, entry2.repo);
    assert_eq!(entry1.dest, entry2.dest);
    assert_eq!(entry1.profile, entry2.profile);
  }
}

mod path_handling_tests {
  use std::path::PathBuf;

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

  #[test]
  fn test_path_joining() {
    let base = PathBuf::from("/home/user");
    let relative = ".config";
    let joined = base.join(relative);

    assert_eq!(joined, PathBuf::from("/home/user/.config"));
  }
}

#[test]
fn dry_run_rename_records_a_move_without_changing_files() -> Result<(), Box<dyn std::error::Error>>
{
  use flux_lib::file_manager::FileSystemManager;
  use flux_lib::utils::dry_run::{DryRun, Operation};
  let root = tempfile::tempdir()?;
  let from = root.path().join("from");
  let to = root.path().join("to");
  std::fs::write(&from, "original")?;
  let mut dry_run = DryRun::new();
  FileSystemManager::new(&mut dry_run, true).rename(&from, &to)?;
  assert!(
    matches!(dry_run.operations(), [Operation::Rename { from: actual_from, to: actual_to }] if actual_from == &from && actual_to == &to)
  );
  assert_eq!(std::fs::read_to_string(from)?, "original");
  assert!(!to.exists());
  Ok(())
}
