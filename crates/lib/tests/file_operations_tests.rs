//! Tests for file operations.

use flux_lib::{
    config::Config,
    file_manager::{FileSystemManager, add_file, remove_file, sync_files},
    utils::dry_run::DryRun,
    utils::home_override,
    utils::path_utils::{normalize_path, resolve_symlink_target},
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
};
use tempfile::{Builder as TempDirBuilder, TempDir, tempdir};

struct Workspace {
    repo_temp: TempDir,
    repo_path: PathBuf,
    home_temp: TempDir,
    home_path: PathBuf,
    lock_guard: MutexGuard<'static, ()>,
}

impl Drop for Workspace {
    fn drop(&mut self) {
        home_override::clear_override();
    }
}

impl Workspace {
    fn repo_path(&self) -> &Path {
        &self.repo_path
    }

    fn home_path(&self) -> &Path {
        &self.home_path
    }
}

/// Create an isolated workspace with the repo rooted in a generic tempdir and the
/// home directory created inside the real HOME (so `dirs::home_dir()` still points
/// inside it for security checks).
static WORKSPACE_LOCK: std::sync::LazyLock<Mutex<()>> = std::sync::LazyLock::new(|| Mutex::new(()));

fn setup_workspace() -> Result<Workspace, Box<dyn std::error::Error>> {
    let guard = WORKSPACE_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let repo_temp = tempdir()?;
    let repo_path = repo_temp.path().join("repo");
    fs::create_dir_all(&repo_path)?;

    let home_temp = TempDirBuilder::new().prefix("flux-test-home").tempdir()?;
    let home_path = home_temp.path().to_path_buf();
    fs::create_dir_all(&home_path)?;
    home_override::set_override(home_path.clone());

    Ok(Workspace {
        repo_temp,
        repo_path,
        home_temp,
        home_path,
        lock_guard: guard,
    })
}

fn prepare_config(repo_path: &Path) -> Config {
    let mut config = Config::default();
    config.general.repo_path = repo_path.to_string_lossy().into_owned();
    config
}

fn add_sample_file(
    config: &mut Config,
    home_path: &Path,
) -> Result<(PathBuf, String), Box<dyn std::error::Error>> {
    let source_dir = home_path.join("existing").join("files");
    fs::create_dir_all(&source_dir)?;
    let source_file = source_dir.join("init.nix");
    fs::write(&source_file, "sample content")?;

    let dest_path = home_path.join(".config").join("flux").join("init.nix");
    if let Some(parent) = dest_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut dry_run_tracker = DryRun::default();
    let mut fs_manager = FileSystemManager::new(&mut dry_run_tracker, false);
    add_file(
        config,
        "flux",
        &source_file,
        &dest_path,
        None,
        &mut fs_manager,
    )?;
    let repo_entry = config.tools["flux"].files[0].repo.clone();
    Ok((dest_path, repo_entry))
}

#[cfg(test)]
mod file_operations_integration_tests {
    use super::*;

    #[test]
    fn add_file_tracks_tool_entry() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = setup_workspace()?;
        let mut config = prepare_config(workspace.repo_path());

        let (dest_path, repo_entry) = add_sample_file(&mut config, workspace.home_path())?;
        let repo_file = workspace.repo_path().join(Path::new(&repo_entry));

        assert!(repo_file.exists(), "Repo should contain copied file");
        assert!(dest_path.parent().is_some_and(Path::exists));

        let tool_config = config.tools.get("flux").expect("tool should be recorded in config");
        assert_eq!(tool_config.files.len(), 1);
        assert_eq!(
            tool_config.files[0].dest,
            dest_path.to_string_lossy(),
            "Destination path should be recorded"
        );
        assert_eq!(tool_config.files[0].repo, repo_entry);
        Ok(())
    }

    #[test]
    fn sync_files_creates_symlink() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = setup_workspace()?;
        let mut config = prepare_config(workspace.repo_path());

        let (dest_path, repo_entry) = add_sample_file(&mut config, workspace.home_path())?;
        let repo_file = workspace.repo_path().join(Path::new(&repo_entry));

        assert!(repo_file.exists());

        let mut dry_run_tracker = DryRun::default();
        sync_files(&config, None, &mut dry_run_tracker, false, false)?;

        assert!(dest_path.is_symlink(), "Sync should create a symlink");
        let symlink_target = fs::read_link(&dest_path)?;
        let resolved_target = resolve_symlink_target(&dest_path, &symlink_target);
        let normalized_target = normalize_path(&resolved_target);
        let normalized_repo = normalize_path(&repo_file);
        assert_eq!(normalized_target, normalized_repo);
        Ok(())
    }

    #[test]
    fn remove_file_cleans_up_artifacts() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = setup_workspace()?;
        let mut config = prepare_config(workspace.repo_path());

        let (dest_path, repo_entry) = add_sample_file(&mut config, workspace.home_path())?;
        let repo_file = workspace.repo_path().join(Path::new(&repo_entry));

        let mut dry_run_tracker = DryRun::default();
        sync_files(&config, None, &mut dry_run_tracker, false, false)?;

        let mut remove_tracker = DryRun::default();
        let mut fs_manager = FileSystemManager::new(&mut remove_tracker, false);
        remove_file(&mut config, "flux", &repo_entry, &mut fs_manager)?;

        assert!(!dest_path.exists());
        assert!(!repo_file.exists());
        assert!(config.tools.contains_key("flux"));

        Ok(())
    }
}

#[cfg(test)]
mod symlink_resolution_tests {
    use super::*;
    use flux_lib::file_manager::FileSystemManager;
    use flux_lib::utils::dry_run::DryRun;
    use pathdiff::diff_paths;

    #[test]
    #[cfg(unix)]
    fn absolute_symlink_via_manager() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = setup_workspace()?;

        let target = workspace.repo_path().join("config");
        fs::write(&target, "content")?;
        let link_path = workspace.home_path().join("absolute_link");

        let mut dry_run_tracker = DryRun::default();
        let mut fs_manager = FileSystemManager::new(&mut dry_run_tracker, false);
        fs_manager.symlink(&target, &link_path)?;

        assert!(link_path.is_symlink());
        let actual = fs::read_link(&link_path)?;
        assert_eq!(actual, target);
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn relative_symlink_via_manager() -> Result<(), Box<dyn std::error::Error>> {
        let workspace = setup_workspace()?;

        let target = workspace.repo_path().join("config");
        fs::write(&target, "content")?;

        let link_parent = workspace.home_path().join("links");
        fs::create_dir_all(&link_parent)?;
        let link_path = link_parent.join("relative_link");

        let relative_target =
            diff_paths(&target, &link_parent).expect("relative path should be computable");

        let mut dry_run_tracker = DryRun::default();
        let mut fs_manager = FileSystemManager::new(&mut dry_run_tracker, false);
        fs_manager.symlink(&relative_target, &link_path)?;

        assert!(link_path.is_symlink());
        let actual = fs::read_link(&link_path)?;
        assert_eq!(actual, relative_target);
        Ok(())
    }
}
