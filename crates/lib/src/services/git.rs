use crate::types::FileChange;
use crate::utils::dry_run::{DryRun, Operation};
use crate::utils::error::{DotfilesError, Result};
use crate::utils::error_utils;
use crate::utils::home_override;
use git2::{CredentialType, RemoteCallbacks, Repository, Signature};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::timeout as tokio_timeout;

/// Encapsulates a git repository with common operations
pub struct GitRepository {
  repo: Repository,
  repo_path: PathBuf,
}

/// Summary of a commit operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitReport {
  /// Short branch name, if one was available.
  pub branch_name: Option<String>,
  /// Short commit id.
  pub short_id: String,
  /// Commit message used.
  pub message: String,
}

/// Information about a configured remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteInfo {
  /// Remote name.
  pub name: String,
  /// Fetch URL.
  pub fetch_url: String,
  /// Push URL.
  pub push_url: String,
}

/// Summary of `git remote -v`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteListReport {
  /// Configured remotes.
  pub remotes: Vec<RemoteInfo>,
}

/// Summary of a push operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PushReport {
  /// Remote name used.
  pub remote: String,
  /// Branch name used.
  pub branch: String,
  /// Remote URL.
  pub remote_url: String,
  /// Whether local and remote were already up to date.
  pub up_to_date: bool,
  /// Whether upstream was configured.
  pub set_upstream: bool,
  /// Short HEAD before the push.
  pub head_before: Option<String>,
  /// Short HEAD after the push.
  pub head_after: Option<String>,
}

/// Summary of a pull operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullReport {
  /// Remote name used.
  pub remote: String,
  /// Branch name used.
  pub branch: String,
  /// Whether the pull was already up to date.
  pub up_to_date: bool,
  /// Whether the merge was fast-forward.
  pub was_fast_forward: bool,
  /// Short HEAD before the pull.
  pub head_before: Option<String>,
  /// Short HEAD after the pull.
  pub head_after: Option<String>,
  /// Files changed by the merge.
  pub files_changed: usize,
  /// Insertions in the merge.
  pub insertions: usize,
  /// Deletions in the merge.
  pub deletions: usize,
}

/// Snapshot of `git status`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatusReport {
  /// Current branch name.
  pub branch_name: Option<String>,
  /// Upstream branch name.
  pub upstream_name: Option<String>,
  /// Short HEAD commit id.
  pub short_id: Option<String>,
  /// First-line commit message.
  pub head_message: Option<String>,
  /// Number of commits ahead of upstream.
  pub ahead: Option<usize>,
  /// Number of commits behind upstream.
  pub behind: Option<usize>,
  /// Current file changes.
  pub changes: Vec<FileChange>,
}

impl std::fmt::Debug for GitRepository {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::fmt::Result {
    f.debug_struct("GitRepository")
      .field("repo_path", &self.repo_path)
      .field("repo", &"<git2::Repository>")
      .finish()
  }
}

impl GitRepository {
  /// Initialize or open a git repository at the given path
  ///
  /// # Errors
  /// Returns an error if the repository cannot be initialized or opened.
  pub fn new(repo_path: &Path) -> Result<Self> {
    let repo = if repo_path.join(".git").exists() {
      Repository::open(repo_path)?
    } else {
      Repository::init(repo_path)?
    };

    if repo.head().is_err() {
      repo.set_head("refs/heads/main")?;
    }

    let repo_path = repo_path.to_path_buf();
    Ok(Self { repo, repo_path })
  }

  /// Open an existing git repository at the given path.
  ///
  /// This does not create a repository if one does not already exist.
  ///
  /// # Errors
  ///
  /// Returns an error if the repository cannot be opened.
  pub fn open_existing(repo_path: &Path) -> Result<Self> {
    let repo = Repository::open(repo_path)?;
    let repo_path = repo_path.to_path_buf();
    Ok(Self { repo, repo_path })
  }

  /// Get the underlying git2 Repository
  #[must_use]
  pub fn inner(&self) -> &Repository {
    &self.repo
  }

  /// Get the repository path
  #[must_use]
  pub fn path(&self) -> &Path {
    &self.repo_path
  }

  /// Get the user's git signature from their git config
  ///
  /// # Errors
  /// Returns an error if the user's git signature cannot be retrieved.
  pub fn get_user_signature(&self) -> Result<Signature<'_>> {
    let config = self.repo.config()?;

    let name = config.get_string("user.name").map_err(|err| {
      if err.code() == git2::ErrorCode::NotFound {
        DotfilesError::Config(
          "Git user.name is not configured. Please set it with: git config --global user.name \"Your Name\""
            .to_string(),
        )
      } else {
        DotfilesError::Config(format!("Failed to read git user.name: {err}"))
      }
    })?;

    let email = config.get_string("user.email").map_err(|err| {
            if err.code() == git2::ErrorCode::NotFound {
                DotfilesError::Config(
                    "Git user.email is not configured. Please set it with: git config --global user.email \"your.email@example.com\"".to_string()
                )
            } else {
                DotfilesError::Config(format!("Failed to read git user.email: {err}"))
            }
        })?;

    Signature::now(&name, &email)
      .map_err(|err| DotfilesError::Config(format!("Failed to create git signature: {err}")))
  }

  /// Get the current branch name
  ///
  /// # Errors
  ///
  /// Returns an error if the current branch cannot be determined
  pub fn get_current_branch(&self) -> Result<String> {
    let head = self.repo.head()?;
    let shorthand = head.shorthand().ok_or_else(|| {
      git2::Error::new(
        git2::ErrorCode::Invalid,
        git2::ErrorClass::Reference,
        "Could not determine current branch",
      )
    })?;
    Ok(shorthand.to_string())
  }

  /// Detect changes in the git repository
  ///
  /// # Errors
  ///
  /// Returns an error if repository status cannot be determined
  pub fn detect_changes(&self) -> Result<Vec<FileChange>> {
    let mut changes = Vec::new();
    let mut status_options = git2::StatusOptions::new();
    let _ = status_options.include_untracked(true);
    let _ = status_options.include_ignored(false);

    let statuses = self.repo.statuses(Some(&mut status_options))?;

    for entry in statuses.iter() {
      let Some(path) = entry.path() else {
        continue;
      };
      let status = entry.status();

      let repo_path = self.repo_path.join(path);

      match () {
        () if status.is_index_new() || status.is_wt_new() => {
          changes.push(FileChange::Added(repo_path));
        }
        () if status.is_index_modified() || status.is_wt_modified() => {
          changes.push(FileChange::Modified(repo_path));
        }
        () if status.is_index_deleted() || status.is_wt_deleted() => {
          changes.push(FileChange::Deleted(repo_path));
        }
        () => {}
      }
    }

    Ok(changes)
  }
}

fn get_user_signature(repo: &Repository) -> Result<Signature<'_>> {
  let config = repo.config()?;

  let name = config
    .get_string("user.name")
    .map_err(|err| match err.code() {
      git2::ErrorCode::NotFound => DotfilesError::Config(
        "Git user.name is not configured. Please set it with: git config --global user.name \"Your Name\"".to_string(),
      ),
      git2::ErrorCode::GenericError
      | git2::ErrorCode::Exists
      | git2::ErrorCode::Ambiguous
      | git2::ErrorCode::BufSize
      | git2::ErrorCode::User
      | git2::ErrorCode::BareRepo
      | git2::ErrorCode::UnbornBranch
      | git2::ErrorCode::Unmerged
      | git2::ErrorCode::NotFastForward
      | git2::ErrorCode::InvalidSpec
      | git2::ErrorCode::Conflict
      | git2::ErrorCode::Locked
      | git2::ErrorCode::Modified
      | git2::ErrorCode::Auth
      | git2::ErrorCode::Certificate
      | git2::ErrorCode::Applied
      | git2::ErrorCode::Peel
      | git2::ErrorCode::Eof
      | git2::ErrorCode::Invalid
      | git2::ErrorCode::Uncommitted
      | git2::ErrorCode::Directory
      | git2::ErrorCode::MergeConflict
      | git2::ErrorCode::HashsumMismatch
      | git2::ErrorCode::IndexDirty
      | git2::ErrorCode::ApplyFail
      | git2::ErrorCode::Owner
      | git2::ErrorCode::Timeout => DotfilesError::Config(format!("Failed to read git user.name: {err}")),
    })?;

  let email =
    config.get_string("user.email").map_err(|err| {
      match err.code() {
    git2::ErrorCode::NotFound => DotfilesError::Config(
      "Git user.email is not configured. Please set it with: git config --global user.email \"your.email@example.com\""
        .to_string(),
    ),
    git2::ErrorCode::GenericError
    | git2::ErrorCode::Exists
    | git2::ErrorCode::Ambiguous
    | git2::ErrorCode::BufSize
    | git2::ErrorCode::User
    | git2::ErrorCode::BareRepo
    | git2::ErrorCode::UnbornBranch
    | git2::ErrorCode::Unmerged
    | git2::ErrorCode::NotFastForward
    | git2::ErrorCode::InvalidSpec
    | git2::ErrorCode::Conflict
    | git2::ErrorCode::Locked
    | git2::ErrorCode::Modified
    | git2::ErrorCode::Auth
    | git2::ErrorCode::Certificate
    | git2::ErrorCode::Applied
    | git2::ErrorCode::Peel
    | git2::ErrorCode::Eof
    | git2::ErrorCode::Invalid
    | git2::ErrorCode::Uncommitted
    | git2::ErrorCode::Directory
    | git2::ErrorCode::MergeConflict
    | git2::ErrorCode::HashsumMismatch
    | git2::ErrorCode::IndexDirty
    | git2::ErrorCode::ApplyFail
    | git2::ErrorCode::Owner
    | git2::ErrorCode::Timeout => DotfilesError::Config(format!("Failed to read git user.email: {err}")),
  }
    })?;

  Signature::now(&name, &email)
    .map_err(|err| DotfilesError::Config(format!("Failed to create git signature: {err}")))
}

fn setup_credential_callbacks(repo: &Repository) -> RemoteCallbacks<'static> {
  let config = repo.config().ok();
  let mut callbacks = RemoteCallbacks::new();
  let _ = callbacks.credentials(move |url, username_from_url, allowed_types| {
    let username = username_from_url.unwrap_or("git");
    trace_git_auth(format_args!(
      "credentials url={url} username_from_url={username_from_url:?} allowed_types={allowed_types:?}"
    ));

    if allowed_types.contains(CredentialType::USERNAME) {
      trace_git_auth(format_args!("return username credential for {username}"));
      return git2::Cred::username(username);
    }

    if allowed_types.contains(CredentialType::SSH_KEY) {
      if let Ok(cred) = git2::Cred::ssh_key_from_agent(username) {
        trace_git_auth(format_args!("using ssh agent for {username}"));
        return Ok(cred);
      }

      if let Some(cred) = try_default_ssh_keys(username) {
        trace_git_auth(format_args!("using default ssh key for {username}"));
        return Ok(cred);
      }
    }

    if allowed_types.contains(CredentialType::USER_PASS_PLAINTEXT) {
      if let (Ok(user), Ok(pass)) = (std::env::var("GIT_USERNAME"), std::env::var("GIT_PASSWORD"))
        && let Ok(cred) = git2::Cred::userpass_plaintext(&user, &pass)
      {
        trace_git_auth(format_args!("using GIT_USERNAME/GIT_PASSWORD"));
        return Ok(cred);
      }

      if let Some(config) = config.as_ref()
        && let Ok(cred) = git2::Cred::credential_helper(config, url, username_from_url)
      {
        trace_git_auth(format_args!("using git credential helper"));
        return Ok(cred);
      }
    }

    if allowed_types.contains(CredentialType::DEFAULT)
      && let Ok(cred) = git2::Cred::default()
    {
      trace_git_auth(format_args!("using default credentials"));
      return Ok(cred);
    }

    trace_git_auth(format_args!("no supported authentication method succeeded"));
    Err(git2::Error::from_str(&format!(
      "No supported authentication method succeeded for {url}"
    )))
  });
  callbacks
}

fn try_default_ssh_keys(username: &str) -> Option<git2::Cred> {
  let mut private_keys = Vec::new();

  if let Ok(path) = std::env::var("GIT_SSH_KEY")
    && !path.is_empty()
  {
    private_keys.push(PathBuf::from(path));
  }

  if let Some(home) = home_override::home_dir() {
    for name in ["id_ed25519", "id_ecdsa", "id_rsa"] {
      let path = home.join(".ssh").join(name);
      if path.exists() {
        private_keys.push(path);
      }
    }
  }

  private_keys.into_iter().find_map(|private_key| {
    trace_git_auth(format_args!("trying ssh key {}", private_key.display()));
    git2::Cred::ssh_key(username, None, &private_key, None).ok()
  })
}

fn trace_git_auth(args: std::fmt::Arguments<'_>) {
  if std::env::var_os("FLUX_GIT_TRACE_AUTH").is_some() {
    log::debug!("[flux git auth] {args}");
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RemoteCheckCacheEntry {
  head_oid: String,
  remote_oid: Option<String>,
  checked_at_unix: u64,
}

const REMOTE_CHECK_CACHE_TTL_SECS: u64 = 60;
const CHILD_WAIT_POLL_INTERVAL_MS: u64 = 50;

async fn execute_with_timeout<F, T>(
  operation: F,
  timeout_seconds: u64,
) -> Result<T>
where
  F: FnOnce() -> Result<T> + Send + 'static,
  T: Send + 'static,
{
  let timeout_duration = Duration::from_secs(timeout_seconds);

  match tokio_timeout(timeout_duration, async {
    tokio::task::spawn_blocking(operation)
      .await
      .map_err(|error| DotfilesError::Config(format!("Task join error: {error}")))?
  })
  .await
  {
    Ok(result) => result,
    Err(_) => Err(DotfilesError::Config(format!(
      "Operation timed out after {timeout_seconds} seconds"
    ))),
  }
}

fn run_command_with_timeout(
  command: &mut Command,
  timeout_seconds: u64,
) -> Result<Option<std::process::Output>> {
  let mut child = command
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|error| DotfilesError::Config(format!("Failed to spawn command: {error}")))?;

  let timeout = Duration::from_secs(timeout_seconds);
  let started = Instant::now();

  loop {
    if let Some(_status) = child
      .try_wait()
      .map_err(|error| DotfilesError::Config(format!("Failed waiting on command: {error}")))?
    {
      let output = child.wait_with_output().map_err(|error| {
        DotfilesError::Config(format!("Failed reading command output: {error}"))
      })?;
      return Ok(Some(output));
    }

    if started.elapsed() >= timeout {
      let _ = child.kill();
      let _ = child.wait();
      return Ok(None);
    }

    std::thread::sleep(Duration::from_millis(CHILD_WAIT_POLL_INTERVAL_MS));
  }
}

fn remote_check_cache_dir() -> Option<PathBuf> {
  let cache_dir = dirs::cache_dir()?;
  Some(cache_dir.join("flux").join("remote-check"))
}

fn remote_check_cache_path(
  repo_path: &Path,
  remote_name: &str,
  branch_name: &str,
) -> Option<PathBuf> {
  let mut hasher = DefaultHasher::new();
  repo_path.hash(&mut hasher);
  remote_name.hash(&mut hasher);
  branch_name.hash(&mut hasher);
  let cache_key = format!("{:016x}", hasher.finish());

  Some(remote_check_cache_dir()?.join(format!("{cache_key}.json")))
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CachedRemoteCheck {
  Miss,
  Hit(Option<String>),
}

fn read_remote_check_cache(
  repo_path: &Path,
  remote_name: &str,
  branch_name: &str,
  head_oid: &str,
) -> CachedRemoteCheck {
  let Some(cache_path) = remote_check_cache_path(repo_path, remote_name, branch_name) else {
    return CachedRemoteCheck::Miss;
  };
  let Ok(content) = fs::read_to_string(cache_path) else {
    return CachedRemoteCheck::Miss;
  };
  let Ok(cache_entry) = serde_json::from_str::<RemoteCheckCacheEntry>(&content) else {
    return CachedRemoteCheck::Miss;
  };

  if cache_entry.head_oid != head_oid {
    return CachedRemoteCheck::Miss;
  }

  let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
    return CachedRemoteCheck::Miss;
  };
  if now
    .as_secs()
    .saturating_sub(cache_entry.checked_at_unix)
    > REMOTE_CHECK_CACHE_TTL_SECS
  {
    return CachedRemoteCheck::Miss;
  }

  CachedRemoteCheck::Hit(cache_entry.remote_oid)
}

fn write_remote_check_cache(
  repo_path: &Path,
  remote_name: &str,
  branch_name: &str,
  head_oid: &str,
  remote_oid: Option<&str>,
) {
  let Some(cache_path) = remote_check_cache_path(repo_path, remote_name, branch_name) else {
    return;
  };
  let Some(cache_dir) = cache_path.parent() else {
    return;
  };

  if fs::create_dir_all(cache_dir).is_err() {
    return;
  }

  let checked_at_unix = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .ok()
    .map_or(0, |duration| duration.as_secs());
  let cache_entry = RemoteCheckCacheEntry {
    head_oid: head_oid.to_string(),
    remote_oid: remote_oid.map(ToString::to_string),
    checked_at_unix,
  };

  let Ok(serialized) = serde_json::to_string(&cache_entry) else {
    return;
  };

  let _ = fs::write(cache_path, serialized);
}

/// Result of checking whether the local repository matches the remote branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RemoteSyncCheck {
  /// The local HEAD matches the remote HEAD.
  UpToDate {
    /// Remote name used for the check.
    remote: String,
    /// Branch name used for the check.
    branch: String,
  },
  /// The local HEAD differs from the remote HEAD.
  NotUpToDate {
    /// Remote name used for the check.
    remote: String,
    /// Branch name used for the check.
    branch: String,
    /// Short local commit hash.
    local_oid: String,
    /// Short remote commit hash.
    remote_oid: String,
  },
  /// The remote could not be checked.
  Unknown,
}

fn short_oid(oid: &str) -> String {
  oid.chars().take(7).collect()
}

fn parse_remote_tracking_ref(reference: &str) -> Option<(String, String)> {
  let stripped = reference.strip_prefix("refs/remotes/")?;
  let (remote, branch) = stripped.split_once('/')?;
  Some((remote.to_string(), branch.to_string()))
}

fn resolve_remote_sync_target(
  git_repo: &GitRepository,
  remote_name: Option<&str>,
  branch_name: Option<&str>,
) -> Option<(String, String)> {
  if let (Some(remote), Some(branch)) = (remote_name, branch_name) {
    return Some((remote.to_string(), branch.to_string()));
  }

  let current_branch = git_repo.get_current_branch().ok();
  let current_branch = branch_name
    .map(ToString::to_string)
    .or(current_branch)?;

  let local_branch = git_repo
    .inner()
    .find_branch(&current_branch, git2::BranchType::Local)
    .ok();

  let mut resolved_remote = remote_name.map(ToString::to_string);
  let mut resolved_branch = branch_name.map(ToString::to_string);

  if let Some(local_branch) = local_branch
    && let Ok(upstream_branch) = local_branch.upstream()
    && let Some(upstream_name) = upstream_branch.get().name()
    && let Some((upstream_remote, upstream_branch_name)) = parse_remote_tracking_ref(upstream_name)
  {
    if resolved_remote.is_none() {
      resolved_remote = Some(upstream_remote);
    }
    if resolved_branch.is_none() {
      resolved_branch = Some(upstream_branch_name);
    }
  }

  Some((resolved_remote?, resolved_branch?))
}

async fn lookup_remote_head_oid(
  repo_path: &Path,
  remote_name: &str,
  branch_name: &str,
  timeout_seconds: u64,
) -> Result<Option<String>> {
  let repo_path = repo_path.to_path_buf();
  let remote_name = remote_name.to_string();
  let branch_name = branch_name.to_string();

  tokio::task::spawn_blocking(move || -> Result<Option<String>> {
    trace_git_auth(format_args!(
      "lookup_remote_head_oid using system git for repo={} remote={} branch={}",
      repo_path.display(),
      remote_name,
      branch_name
    ));

    let ssh_command =
      format!("ssh -o BatchMode=yes -o ConnectTimeout={timeout_seconds} -o ConnectionAttempts=1");
    let mut command = Command::new("git");
    let _ = command
      .arg("-C")
      .arg(&repo_path)
      .arg("ls-remote")
      .arg("--heads")
      .arg(&remote_name)
      .arg(&branch_name)
      .env("GIT_TERMINAL_PROMPT", "0")
      .env("GIT_ASKPASS", "true")
      .env("SSH_ASKPASS", "true")
      .env("GIT_SSH_COMMAND", &ssh_command);

    let Some(output) = run_command_with_timeout(&mut command, timeout_seconds)? else {
      return Ok(None);
    };

    if !output.status.success() {
      return Ok(None);
    }

    let remote_oid = String::from_utf8_lossy(&output.stdout)
      .lines()
      .find_map(|line| line.split_whitespace().next())
      .map(ToString::to_string);

    Ok(remote_oid)
  })
  .await
  .map_err(|error| DotfilesError::Config(format!("Task join error: {error}")))?
}

/// Check whether the local branch matches the remote branch without updating refs.
///
/// This is designed for shell hooks: it stays silent when the remote cannot be
/// reached and only reports when the local repository differs from the remote.
///
/// # Errors
///
/// Returns `RemoteSyncCheck::Unknown` if the remote cannot be reached or if the
/// local and remote branches do not match.
pub async fn check_remote_sync(
  git_repo: &GitRepository,
  remote_name: Option<&str>,
  branch_name: Option<&str>,
  timeout_seconds: u64,
) -> Result<RemoteSyncCheck> {
  let Some((resolved_remote, resolved_branch)) =
    resolve_remote_sync_target(git_repo, remote_name, branch_name)
  else {
    return Ok(RemoteSyncCheck::Unknown);
  };

  let head_oid = match git_repo.inner().head() {
    Ok(head) => match head.peel_to_commit() {
      Ok(commit) => commit.id().to_string(),
      Err(_) => return Ok(RemoteSyncCheck::Unknown),
    },
    Err(_) => return Ok(RemoteSyncCheck::Unknown),
  };

  let remote_oid = match read_remote_check_cache(
    &git_repo.repo_path,
    &resolved_remote,
    &resolved_branch,
    &head_oid,
  ) {
    CachedRemoteCheck::Hit(cached_remote_oid) => cached_remote_oid,
    CachedRemoteCheck::Miss => {
      let looked_up_remote_oid = lookup_remote_head_oid(
        &git_repo.repo_path,
        &resolved_remote,
        &resolved_branch,
        timeout_seconds,
      )
      .await
      .ok()
      .flatten();

      write_remote_check_cache(
        &git_repo.repo_path,
        &resolved_remote,
        &resolved_branch,
        &head_oid,
        looked_up_remote_oid.as_deref(),
      );

      looked_up_remote_oid
    }
  };

  let Some(remote_oid) = remote_oid else {
    return Ok(RemoteSyncCheck::Unknown);
  };

  if remote_oid.is_empty() {
    return Ok(RemoteSyncCheck::Unknown);
  }

  if head_oid == remote_oid {
    Ok(RemoteSyncCheck::UpToDate {
      remote: resolved_remote,
      branch: resolved_branch,
    })
  } else {
    Ok(RemoteSyncCheck::NotUpToDate {
      remote: resolved_remote,
      branch: resolved_branch,
      local_oid: short_oid(&head_oid),
      remote_oid: short_oid(&remote_oid),
    })
  }
}

/// Initialize or open a git repository at the given path.
///
/// # Errors
///
/// Returns an error if the repository cannot be initialized or opened.
pub fn init_repo(repo_path: &Path) -> Result<GitRepository> {
  GitRepository::new(repo_path)
}

/// Stage file changes in the git repository.
///
/// # Errors
///
/// Returns an error if changes cannot be staged.
pub fn stage_changes(
  git_repo: &GitRepository,
  changes: &[FileChange],
  dry_run: &mut DryRun,
  is_dry_run: bool,
) -> Result<()> {
  if is_dry_run {
    let files: Vec<_> = changes
      .iter()
      .map(|change| match change {
        FileChange::Added(path) | FileChange::Modified(path) | FileChange::Deleted(path) => {
          path.clone()
        }
      })
      .collect();
    dry_run.log_operation(Operation::GitStage { files });
    return Ok(());
  }

  let mut index = git_repo.inner().index()?;
  let repo_path = &git_repo.repo_path;

  for change in changes {
    match change {
      FileChange::Added(path) | FileChange::Modified(path) => {
        if path.is_dir() {
          for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(std::result::Result::ok)
          {
            if entry.path().is_file()
              && let Ok(relative) = entry.path().strip_prefix(repo_path)
            {
              index.add_path(relative)?;
            }
          }
        } else if let Ok(relative) = path.strip_prefix(repo_path) {
          index.add_path(relative)?;
        }
      }
      FileChange::Deleted(path) => {
        if path.is_dir() {
          for entry in walkdir::WalkDir::new(path)
            .into_iter()
            .filter_map(std::result::Result::ok)
          {
            if entry.path().is_file()
              && let Ok(relative) = entry.path().strip_prefix(repo_path)
            {
              index.remove_path(relative)?;
            }
          }
        } else if let Ok(relative) = path.strip_prefix(repo_path) {
          index.remove_path(relative)?;
        }
      }
    }
  }

  index.write()?;

  Ok(())
}

/// Commit staged changes in git repository.
///
/// # Errors
///
/// Returns an error if changes cannot be committed.
pub fn commit_changes(
  git_repo: &GitRepository,
  commit_message: &str,
  dry_run: &mut DryRun,
  is_dry_run: bool,
) -> Result<CommitReport> {
  if is_dry_run {
    dry_run.log_operation(Operation::GitCommit {
      message: commit_message.to_string(),
    });
    return Ok(CommitReport {
      branch_name: None,
      short_id: String::new(),
      message: commit_message.to_string(),
    });
  }

  let mut index = git_repo.inner().index()?;
  let tree_id = index.write_tree()?;
  let tree = git_repo.inner().find_tree(tree_id)?;

  let signature = git_repo.get_user_signature()?;

  let head = git_repo.inner().head();
  let parent_commit = if let Ok(head) = head {
    Some(head.peel_to_commit()?)
  } else {
    None
  };

  let parents: Vec<&git2::Commit> = parent_commit.iter().collect();

  let _ = git_repo.inner().commit(
    Some("HEAD"),
    &signature,
    &signature,
    commit_message,
    &tree,
    &parents,
  )?;

  // After committing, refresh the index to ensure it's in sync with the committed tree
  // This prevents old staged entries from showing up as changed on the next status check
  let mut new_index = git_repo.inner().index()?;
  new_index.read_tree(&tree)?;
  new_index.write()?;

  let commit_id = git_repo
    .inner()
    .head()?
    .target()
    .ok_or_else(|| DotfilesError::Config("Could not get commit ID after commit".to_string()))?;
  let short_id = short_oid(&commit_id.to_string());
  let branch_name = git_repo.get_current_branch().ok();

  Ok(CommitReport {
    branch_name,
    short_id,
    message: commit_message.to_string(),
  })
}

/// Detect changes in the git repository.
///
/// # Errors
///
/// Returns an error if the repository status cannot be determined.
pub fn detect_changes(git_repo: &GitRepository) -> Result<Vec<FileChange>> {
  git_repo.detect_changes()
}

/// Get the current branch name (shorthand of HEAD).
///
/// # Errors
///
/// Returns an error if the current branch cannot be determined.
pub fn get_current_branch(git_repo: &GitRepository) -> Result<String> {
  git_repo.get_current_branch()
}

/// Add a remote to the repository.
///
/// # Errors
///
/// Returns an error if the remote cannot be added.
pub fn add_remote(
  git_repo: &GitRepository,
  name: &str,
  url: &str,
  dry_run: &mut DryRun,
  is_dry_run: bool,
) -> Result<()> {
  if is_dry_run {
    dry_run.log_operation(Operation::GitRemoteAdd {
      name: name.to_string(),
      url: url.to_string(),
    });
    return Ok(());
  }

  let _ = git_repo.inner().remote(name, url)?;
  Ok(())
}

/// Remove a remote from the repository.
///
/// # Errors
///
/// Returns an error if the remote cannot be removed.
pub fn remove_remote(
  git_repo: &GitRepository,
  name: &str,
  dry_run: &mut DryRun,
  is_dry_run: bool,
) -> Result<()> {
  if is_dry_run {
    dry_run.log_operation(Operation::GitRemoteRemove {
      name: name.to_string(),
    });
    return Ok(());
  }

  git_repo.inner().remote_delete(name)?;
  Ok(())
}

/// Set or update a remote URL.
///
/// # Errors
///
/// Returns an error if the remote URL cannot be set.
pub fn set_remote_url(
  git_repo: &GitRepository,
  name: &str,
  url: &str,
  dry_run: &mut DryRun,
  is_dry_run: bool,
) -> Result<()> {
  if is_dry_run {
    dry_run.log_operation(Operation::GitRemoteSetUrl {
      name: name.to_string(),
      url: url.to_string(),
    });
    return Ok(());
  }

  git_repo.inner().remote_set_url(name, url)?;
  Ok(())
}

/// List all remotes in the repository (like `git remote -v`).
///
/// # Errors
///
/// Returns an error if remotes cannot be listed.
pub fn list_remotes(git_repo: &GitRepository) -> Result<RemoteListReport> {
  let remotes = git_repo.inner().remotes()?;
  let mut report = RemoteListReport::default();

  for name in &remotes {
    if let Some(remote_name) = name
      && let Ok(remote) = git_repo.inner().find_remote(remote_name)
    {
      let fetch_url = remote
        .url()
        .unwrap_or("(invalid URL)")
        .to_string();
      let push_url = remote.pushurl().unwrap_or(&fetch_url).to_string();
      report.remotes.push(RemoteInfo {
        name: remote_name.to_string(),
        fetch_url,
        push_url,
      });
    }
  }

  Ok(report)
}

async fn execute_push_operation(
  repo_path: &Path,
  remote_name: &str,
  branch_name: &str,
  timeout_seconds: u64,
) -> Result<()> {
  let repo_path_clone = repo_path.to_path_buf();
  let remote_name_clone = remote_name.to_string();
  let refspec = format!("refs/heads/{branch_name}:refs/heads/{branch_name}");
  let refspec_clone = refspec;

  execute_with_timeout(
    move || -> Result<()> {
      let repo_in_thread = Repository::open(&repo_path_clone)?;
      let mut remote = repo_in_thread.find_remote(&remote_name_clone)?;
      let mut push_options_in_thread = git2::PushOptions::new();
      let callbacks_in_thread = setup_credential_callbacks(&repo_in_thread);
      let _ = push_options_in_thread.remote_callbacks(callbacks_in_thread);
      remote.push(&[&refspec_clone], Some(&mut push_options_in_thread))?;
      Ok(())
    },
    timeout_seconds,
  )
  .await
}

fn is_push_up_to_date(
  repo: &Repository,
  remote_name: &str,
  branch_name: &str,
  head_before: Option<git2::Oid>,
  remote_ref_before: Option<git2::Oid>,
) -> bool {
  let remote_branch_ref = format!("refs/remotes/{remote_name}/{branch_name}");
  let remote_ref_after = repo
    .find_reference(&remote_branch_ref)
    .ok()
    .and_then(|ref_| ref_.target());

  if let (Some(before), Some(after)) = (remote_ref_before, remote_ref_after) {
    before == after && head_before == Some(before)
  } else {
    false
  }
}

fn set_upstream_branch(
  repo: &Repository,
  remote_name: &str,
  branch_name: &str,
) -> Result<()> {
  let mut config = repo.config()?;
  config.set_str(&format!("branch.{branch_name}.remote"), remote_name)?;
  config.set_str(
    &format!("branch.{branch_name}.merge"),
    &format!("refs/heads/{branch_name}"),
  )?;
  Ok(())
}

/// Push to a remote repository.
///
/// # Errors
///
/// Returns an error if the push operation fails or times out.
pub async fn push_to_remote(
  git_repo: &GitRepository,
  remote_name: &str,
  branch_name: &str,
  set_upstream: bool,
  timeout_seconds: u64,
  dry_run: &mut DryRun,
  is_dry_run: bool,
) -> Result<PushReport> {
  if is_dry_run {
    dry_run.log_operation(Operation::GitPush {
      remote: remote_name.to_string(),
      branch: branch_name.to_string(),
      set_upstream,
    });
    return Ok(PushReport {
      remote: remote_name.to_string(),
      branch: branch_name.to_string(),
      remote_url: String::new(),
      up_to_date: false,
      set_upstream,
      head_before: None,
      head_after: None,
    });
  }

  let remote = git_repo.inner().find_remote(remote_name)?;
  let remote_url = remote.url().unwrap_or("unknown");

  let head_before = git_repo
    .inner()
    .head()
    .ok()
    .and_then(|head| head.target());
  let remote_branch_ref = format!("refs/remotes/{remote_name}/{branch_name}");
  let remote_ref_before = git_repo
    .inner()
    .find_reference(&remote_branch_ref)
    .ok()
    .and_then(|ref_| ref_.target());

  let start_time = Instant::now();
  let push_result = execute_push_operation(
    &git_repo.repo_path,
    remote_name,
    branch_name,
    timeout_seconds,
  )
  .await;
  let _elapsed = start_time.elapsed();

  match push_result {
    Ok(()) => {
      let is_up_to_date = is_push_up_to_date(
        git_repo.inner(),
        remote_name,
        branch_name,
        head_before,
        remote_ref_before,
      );
      if set_upstream {
        set_upstream_branch(git_repo.inner(), remote_name, branch_name)?;
      }
      let head_after = git_repo
        .inner()
        .head()
        .ok()
        .and_then(|head| head.target())
        .map(|oid| short_oid(&oid.to_string()));
      let head_before = head_before.map(|oid| short_oid(&oid.to_string()));
      Ok(PushReport {
        remote: remote_name.to_string(),
        branch: branch_name.to_string(),
        remote_url: remote_url.to_string(),
        up_to_date: is_up_to_date,
        set_upstream,
        head_before,
        head_after,
      })
    }
    Err(err) => {
      if err.to_string().contains("timed out") {
        Err(DotfilesError::Config(format!(
          "Push operation timed out after {timeout_seconds} seconds"
        )))
      } else {
        let error_msg = format!("{err}");
        Err(error_utils::git_operation_failed(
          "push",
          &git_repo.repo_path,
          &error_msg,
        ))
      }
    }
  }
}

async fn execute_fetch_operation(
  repo_path: &Path,
  remote_name: &str,
  branch_name: &str,
  timeout_seconds: u64,
) -> Result<()> {
  let repo_path_clone = repo_path.to_path_buf();
  let remote_name_clone = remote_name.to_string();
  let branch_name_clone = branch_name.to_string();

  execute_with_timeout(
    move || -> Result<()> {
      trace_git_auth(format_args!(
        "execute_fetch_operation using system git for repo={} remote={} branch={}",
        repo_path_clone.display(),
        remote_name_clone,
        branch_name_clone
      ));
      let refspec_in_thread = format!(
        "refs/heads/{branch_name_clone}:refs/remotes/{remote_name_clone}/{branch_name_clone}"
      );
      let output = Command::new("git")
        .arg("-C")
        .arg(&repo_path_clone)
        .arg("fetch")
        .arg(&remote_name_clone)
        .arg(&refspec_in_thread)
        .output()
        .map_err(|err| DotfilesError::Config(format!("Failed to invoke git fetch: {err}")))?;

      if output.status.success() {
        Ok(())
      } else {
        let stderr = String::from_utf8_lossy(&output.stderr)
          .trim()
          .to_string();
        let stdout = String::from_utf8_lossy(&output.stdout)
          .trim()
          .to_string();
        let message = if !stderr.is_empty() {
          stderr
        } else if !stdout.is_empty() {
          stdout
        } else {
          format!("git fetch exited with status {}", output.status)
        };
        Err(DotfilesError::Config(message))
      }
    },
    timeout_seconds,
  )
  .await
}

fn perform_merge(
  repo: &Repository,
  remote_name: &str,
  branch_name: &str,
  remote_branch: &git2::Reference,
  remote_commit: &git2::Commit,
) -> Result<bool> {
  let annotated_commit = repo.reference_to_annotated_commit(remote_branch)?;
  let (analysis, _) = repo.merge_analysis(&[&annotated_commit])?;

  if analysis.is_up_to_date() {
    return Ok(false);
  }

  if analysis.is_fast_forward() {
    let mut ref_ = repo.find_reference(&format!("refs/heads/{branch_name}"))?;
    let _ = ref_.set_target(remote_commit.id(), "Fast-forward")?;
    repo.set_head(&format!("refs/heads/{branch_name}"))?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
    Ok(true)
  } else {
    let signature = get_user_signature(repo)?;
    repo.merge(&[&annotated_commit], None, None)?;

    check_merge_conflicts(repo)?;

    let mut index = repo.index()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;
    let head = repo.head()?.peel_to_commit()?;
    let _ = repo.commit(
      Some("HEAD"),
      &signature,
      &signature,
      &format!("Merge {remote_name}/{branch_name}"),
      &tree,
      &[&head, remote_commit],
    )?;
    Ok(false)
  }
}

fn calculate_file_stats(
  repo: &Repository,
  old_commit: &git2::Commit,
  new_commit: &git2::Commit,
) -> (usize, usize, usize) {
  let Ok(old_tree) = old_commit.tree() else {
    return (0, 0, 0);
  };
  let Ok(new_tree) = new_commit.tree() else {
    return (0, 0, 0);
  };

  let Ok(diff) = repo.diff_tree_to_tree(Some(&old_tree), Some(&new_tree), None) else {
    return (0, 0, 0);
  };

  let Ok(stats) = diff.stats() else {
    return (0, 0, 0);
  };

  (stats.files_changed(), stats.insertions(), stats.deletions())
}

fn short_commit_id(commit: Option<&git2::Commit>) -> Option<String> {
  commit.map(|commit| short_oid(&commit.id().to_string()))
}

struct PullReportParts {
  remote_name: String,
  branch_name: String,
  up_to_date: bool,
  was_fast_forward: bool,
  head_before: Option<String>,
  head_after: Option<String>,
  files_changed: usize,
  insertions: usize,
  deletions: usize,
}

fn build_pull_report(parts: PullReportParts) -> PullReport {
  PullReport {
    remote: parts.remote_name,
    branch: parts.branch_name,
    up_to_date: parts.up_to_date,
    was_fast_forward: parts.was_fast_forward,
    head_before: parts.head_before,
    head_after: parts.head_after,
    files_changed: parts.files_changed,
    insertions: parts.insertions,
    deletions: parts.deletions,
  }
}

fn pull_timeout_error(timeout_seconds: u64) -> DotfilesError {
  DotfilesError::Config(format!(
    "Pull operation timed out after {timeout_seconds} seconds"
  ))
}

fn pull_fetch_error(
  git_repo: &GitRepository,
  err: &DotfilesError,
  timeout_seconds: u64,
) -> DotfilesError {
  if err.to_string().contains("timed out") {
    pull_timeout_error(timeout_seconds)
  } else {
    let error_msg = format!("{err}");
    error_utils::git_operation_failed("pull", &git_repo.repo_path, &error_msg)
  }
}

fn complete_pull_after_fetch(
  git_repo: &GitRepository,
  remote_name: &str,
  branch_name: &str,
  head_before: Option<&git2::Commit<'_>>,
) -> Result<PullReport> {
  let remote_branch_ref = format!("refs/remotes/{remote_name}/{branch_name}");
  let remote_branch = git_repo
    .inner()
    .find_reference(&remote_branch_ref)
    .map_err(|_| {
      DotfilesError::Config(format!(
        "Could not find remote branch {remote_name}/{branch_name} after fetch"
      ))
    })?;

  let remote_commit = remote_branch.peel_to_commit()?;
  let remote_oid = remote_commit.id();

  if let Some(head_oid) = head_before.map(git2::Commit::id)
    && head_oid == remote_oid
  {
    return Ok(build_pull_report(PullReportParts {
      remote_name: remote_name.to_string(),
      branch_name: branch_name.to_string(),
      up_to_date: true,
      was_fast_forward: false,
      head_before: short_commit_id(head_before),
      head_after: short_commit_id(head_before),
      files_changed: 0,
      insertions: 0,
      deletions: 0,
    }));
  }

  let was_fast_forward = perform_merge(
    git_repo.inner(),
    remote_name,
    branch_name,
    &remote_branch,
    &remote_commit,
  )?;

  let head_after = git_repo
    .inner()
    .head()
    .ok()
    .and_then(|head| head.peel_to_commit().ok());
  let head_oid_after = head_after.as_ref().map(git2::Commit::id);

  if let (Some(oid_before), Some(oid_after)) = (head_before.map(git2::Commit::id), head_oid_after)
    && oid_before == oid_after
  {
    return Ok(build_pull_report(PullReportParts {
      remote_name: remote_name.to_string(),
      branch_name: branch_name.to_string(),
      up_to_date: true,
      was_fast_forward,
      head_before: short_commit_id(head_before),
      head_after: short_commit_id(head_after.as_ref()),
      files_changed: 0,
      insertions: 0,
      deletions: 0,
    }));
  }

  let (files_changed, insertions, deletions) = match (head_before, &head_after) {
    (Some(before), Some(after)) => calculate_file_stats(git_repo.inner(), before, after),
    _ => (0, 0, 0),
  };

  Ok(build_pull_report(PullReportParts {
    remote_name: remote_name.to_string(),
    branch_name: branch_name.to_string(),
    up_to_date: false,
    was_fast_forward,
    head_before: short_commit_id(head_before),
    head_after: short_commit_id(head_after.as_ref()),
    files_changed,
    insertions,
    deletions,
  }))
}

fn check_merge_conflicts(repo: &Repository) -> Result<()> {
  let index = repo.index()?;
  if index.has_conflicts() {
    let conflicts: Vec<String> = index
      .conflicts()?
      .filter_map(|conflict| {
        conflict.ok().and_then(|conf| {
          conf.our.map(|entry| {
            std::str::from_utf8(&entry.path).map_or_else(|_| String::new(), ToString::to_string)
          })
        })
      })
      .collect();

    if !conflicts.is_empty() {
      return Err(DotfilesError::Config(format!(
        "Merge conflicts detected in:\n  {}\n\nTo resolve:\n  1. Resolve conflicts manually\n  2. Stage resolved files: git add <files>\n  3. Complete merge: git commit",
        conflicts.join("\n  ")
      )));
    }
  }

  let statuses = repo.statuses(Some(
    &mut git2::StatusOptions::new().include_untracked(true),
  ))?;
  let blocking_files: Vec<String> = statuses
    .iter()
    .filter_map(|entry| {
      if entry.status().is_wt_new() {
        entry.path().map(ToString::to_string)
      } else {
        None
      }
    })
    .collect();

  if !blocking_files.is_empty() {
    return Err(DotfilesError::Config(format!(
      "Untracked files would be overwritten by merge:\n  {}\n\nTo resolve:\n  1. Backup the files: mv {} <backup-location>\n  2. Run 'flux pull' again\n  3. Compare and merge changes if needed",
      blocking_files.join("\n  "),
      blocking_files.join(" ")
    )));
  }

  Ok(())
}

/// Pull from a remote repository.
///
/// # Errors
///
/// Returns an error if the pull operation fails, times out, or merge conflicts occur.
pub async fn pull_from_remote(
  git_repo: &GitRepository,
  remote_name: &str,
  branch_name: &str,
  timeout_seconds: u64,
  dry_run: &mut DryRun,
  is_dry_run: bool,
) -> Result<PullReport> {
  if is_dry_run {
    dry_run.log_operation(Operation::GitPull {
      remote: remote_name.to_string(),
      branch: branch_name.to_string(),
    });
    return Ok(build_pull_report(PullReportParts {
      remote_name: remote_name.to_string(),
      branch_name: branch_name.to_string(),
      up_to_date: false,
      was_fast_forward: false,
      head_before: None,
      head_after: None,
      files_changed: 0,
      insertions: 0,
      deletions: 0,
    }));
  }

  let remote = git_repo.inner().find_remote(remote_name)?;
  let _remote_url = remote.url().unwrap_or("unknown");

  let head_before = git_repo
    .inner()
    .head()
    .ok()
    .and_then(|head| head.peel_to_commit().ok());

  let _start_time = Instant::now();
  trace_git_auth(format_args!(
    "pull_from_remote start repo={} remote={} branch={}",
    git_repo.repo_path.display(),
    remote_name,
    branch_name
  ));
  let fetch_result = execute_fetch_operation(
    &git_repo.repo_path,
    remote_name,
    branch_name,
    timeout_seconds,
  )
  .await;

  match fetch_result {
    Ok(()) => complete_pull_after_fetch(git_repo, remote_name, branch_name, head_before.as_ref()),
    Err(err) => Err(pull_fetch_error(git_repo, &err, timeout_seconds)),
  }
}

/// Show git repository status (like `git status`).
///
/// # Errors
///
/// Returns an error if the repository status cannot be determined.
pub fn show_git_status(git_repo: &GitRepository) -> Result<GitStatusReport> {
  use git2::BranchType;

  let Ok(branch_name) = get_current_branch(git_repo) else {
    return Ok(GitStatusReport {
      branch_name: None,
      upstream_name: None,
      short_id: None,
      head_message: None,
      ahead: None,
      behind: None,
      changes: Vec::new(),
    });
  };

  let branch = git_repo
    .inner()
    .find_branch(&branch_name, BranchType::Local)?;
  let upstream = branch.upstream();

  let head = git_repo.inner().head()?;
  let head_commit = head.peel_to_commit()?;
  let head_oid = head_commit.id();
  let short_id = head_oid.to_string()[..7].to_string();

  let changes = detect_changes(git_repo)?;

  let (ahead, behind, upstream_name) = calculate_upstream_status(git_repo, head_oid, upstream)?;

  Ok(GitStatusReport {
    branch_name: Some(branch_name),
    upstream_name,
    short_id: Some(short_id),
    head_message: Some(
      head_commit
        .message()
        .unwrap_or("(no message)")
        .lines()
        .next()
        .unwrap_or("(no message)")
        .to_string(),
    ),
    ahead,
    behind,
    changes,
  })
}

fn calculate_upstream_status(
  git_repo: &GitRepository,
  head_oid: git2::Oid,
  upstream: std::result::Result<git2::Branch<'_>, git2::Error>,
) -> Result<(Option<usize>, Option<usize>, Option<String>)> {
  if let Ok(upstream_branch) = upstream {
    let upstream_oid = upstream_branch.get().target().ok_or_else(|| {
      git2::Error::new(
        git2::ErrorCode::NotFound,
        git2::ErrorClass::Reference,
        "Could not find upstream target",
      )
    })?;

    let (ahead, behind) = git_repo
      .inner()
      .graph_ahead_behind(head_oid, upstream_oid)?;
    let upstream_name = upstream_branch
      .get()
      .name()
      .map_or_else(|| "unknown".to_string(), ToString::to_string);
    Ok((Some(ahead), Some(behind), Some(upstream_name)))
  } else {
    Ok((None, None, None))
  }
}
