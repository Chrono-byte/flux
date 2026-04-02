use crate::types::FileChange;
use crate::utils::dry_run::{DryRun, Operation};
use crate::utils::error::{DotfilesError, Result};
use crate::utils::error_utils;
use colored::Colorize;
use git2::{CredentialType, RemoteCallbacks, Repository, Signature};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use tokio::time::timeout as tokio_timeout;

/// Encapsulates a git repository with common operations
pub struct GitRepository {
    /// The underlying git2 Repository
    repo: Repository,
    /// The path to the repository
    repo_path: PathBuf,
}

impl std::fmt::Debug for GitRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

        // Ensure the repository has a valid initial setup
        if repo.head().is_err() {
            repo.set_head("refs/heads/main")?;
        }

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
                    "Git user.name is not configured. Please set it with: git config --global user.name \"Your Name\"".to_string()
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
                () => {} // No change detected
            }
        }

        Ok(changes)
    }
}

/// Get the user's git signature from their git config
/// This reads from the repository's config, which includes global git config
fn get_user_signature(repo: &Repository) -> Result<Signature<'_>> {
    let config = repo.config()?;

    let name = config.get_string("user.name").map_err(|err| {
        match err.code() {
            git2::ErrorCode::NotFound => DotfilesError::Config(
                "Git user.name is not configured. Please set it with: git config --global user.name \"Your Name\"".to_string()
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
            | git2::ErrorCode::Timeout => {
                DotfilesError::Config(format!("Failed to read git user.name: {err}"))
            }
        }
    })?;

    let email = config.get_string("user.email").map_err(|err| {
        match err.code() {
            git2::ErrorCode::NotFound => DotfilesError::Config(
                "Git user.email is not configured. Please set it with: git config --global user.email \"your.email@example.com\"".to_string()
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
            | git2::ErrorCode::Timeout => {
                DotfilesError::Config(format!("Failed to read git user.email: {err}"))
            }
        }
    })?;

    Signature::now(&name, &email)
        .map_err(|err| DotfilesError::Config(format!("Failed to create git signature: {err}")))
}

/// Set up credential callbacks for git2 operations.
/// Handles SSH via username negotiation plus SSH agent, and HTTPS via
/// explicit environment variables or the configured git credential helper.
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
            if let (Ok(user), Ok(pass)) =
                (std::env::var("GIT_USERNAME"), std::env::var("GIT_PASSWORD"))
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

/// Tries to load default SSH keys for the given username.
fn try_default_ssh_keys(username: &str) -> Option<git2::Cred> {
    let mut private_keys = Vec::new();

    if let Ok(path) = std::env::var("GIT_SSH_KEY")
        && !path.is_empty()
    {
        private_keys.push(PathBuf::from(path));
    }

    if let Some(home) = dirs::home_dir() {
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

/// Traces git authentication attempts if the `FLUX_GIT_TRACE_AUTH` environment variable is set.
fn trace_git_auth(args: std::fmt::Arguments<'_>) {
    if std::env::var_os("FLUX_GIT_TRACE_AUTH").is_some() {
        eprintln!("[flux git auth] {args}");
    }
}

/// Execute a git2 operation with a timeout using async/await
async fn execute_with_timeout<F, T>(operation: F, timeout_seconds: u64) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    let timeout_duration = Duration::from_secs(timeout_seconds);

    match tokio_timeout(timeout_duration, async {
        // Run the blocking git2 operation in a thread
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
                FileChange::Added(path)
                | FileChange::Modified(path)
                | FileChange::Deleted(path) => path.clone(),
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
                // If it's a directory, recursively add all files in it
                if path.is_dir() {
                    for entry in
                        walkdir::WalkDir::new(path).into_iter().filter_map(std::result::Result::ok)
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
                // If it's a directory, recursively remove all files in it
                if path.is_dir() {
                    for entry in
                        walkdir::WalkDir::new(path).into_iter().filter_map(std::result::Result::ok)
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

    // Write the index to disk to make sure staged changes are persisted
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
) -> Result<()> {
    if is_dry_run {
        dry_run.log_operation(Operation::GitCommit {
            message: commit_message.to_string(),
        });
        return Ok(());
    }

    // Get the current index (which has been staged by stage_changes)
    let mut index = git_repo.inner().index()?;
    let tree_id = index.write_tree()?;
    let tree = git_repo.inner().find_tree(tree_id)?;

    // Use the user's git config for commit signature
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

    // Get the commit we just created
    let commit_id =
        git_repo.inner().head()?.target().ok_or_else(|| {
            DotfilesError::Config("Could not get commit ID after commit".to_string())
        })?;
    let short_id = commit_id.to_string()[..7].to_string();

    // Get branch name (or "detached HEAD" if not on a branch)
    let branch_name = git_repo.get_current_branch().unwrap_or_else(|_| "detached HEAD".to_string());

    // Show git-like output: [branch-name abc1234] commit message
    println!(
        "[{} {}] {}",
        branch_name.cyan(),
        short_id.cyan(),
        commit_message
    );
    Ok(())
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
    println!("{} Added remote '{}': {}", "✓".green(), name, url);
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
    println!("{} Removed remote '{}'", "✓".green(), name);
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
    println!("{} Set URL for remote '{}': {}", "✓".green(), name, url);
    Ok(())
}

/// List all remotes in the repository (like `git remote -v`).
///
/// # Errors
///
/// Returns an error if remotes cannot be listed.
pub fn list_remotes(git_repo: &GitRepository) -> Result<()> {
    let remotes = git_repo.inner().remotes()?;

    if remotes.is_empty() {
        println!("{} No remotes configured.", "⊘".yellow());
        return Ok(());
    }

    println!();
    for name in &remotes {
        if let Some(remote_name) = name
            && let Ok(remote) = git_repo.inner().find_remote(remote_name)
        {
            let fetch_url = remote.url().unwrap_or("(invalid URL)");
            let push_url = remote.pushurl();

            // Show fetch URL
            println!("{}  {} (fetch)", remote_name.cyan(), fetch_url);

            // Show push URL (use fetch URL if push URL not set, or show if different)
            let actual_push_url = push_url.unwrap_or(fetch_url);
            println!("{}  {} (push)", remote_name.cyan(), actual_push_url);
        }
    }
    println!();
    Ok(())
}

/// Execute a git push operation with timeout.
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

/// Check if push was up-to-date by comparing refs.
fn is_push_up_to_date(
    repo: &Repository,
    remote_name: &str,
    branch_name: &str,
    head_before: Option<git2::Oid>,
    remote_ref_before: Option<git2::Oid>,
) -> bool {
    let remote_branch_ref = format!("refs/remotes/{remote_name}/{branch_name}");
    let remote_ref_after =
        repo.find_reference(&remote_branch_ref).ok().and_then(|ref_| ref_.target());

    if let (Some(before), Some(after)) = (remote_ref_before, remote_ref_after) {
        before == after && head_before == Some(before)
    } else {
        false
    }
}

/// Display push success message with git-like ref update output.
fn display_push_success(
    repo: &Repository,
    branch_name: &str,
    _remote_name: &str,
    remote_url: &str,
    head_before: Option<git2::Oid>,
    _remote_ref_before: Option<git2::Oid>,
) {
    // Git-like output: "To <url>"
    println!("To {remote_url}");

    // Get current HEAD after push
    let head_after = repo.head().ok().and_then(|head| head.target());

    // Show ref update in git format: "abc1234..def5678  branch -> branch"
    if let (Some(before), Some(after)) = (head_before, head_after) {
        if before == after {
            // New branch (no before)
            let after_short = after.to_string()[..7].to_string();
            println!("   {after_short}  {branch_name} -> {branch_name} (new branch)");
        } else {
            let before_short = before.to_string()[..7].to_string();
            let after_short = after.to_string()[..7].to_string();
            println!("   {before_short}..{after_short}  {branch_name} -> {branch_name}");
        }
    } else if let Some(after) = head_after {
        // New branch
        let after_short = after.to_string()[..7].to_string();
        println!("   {after_short}  {branch_name} -> {branch_name} (new branch)");
    }
}

/// Set upstream branch configuration.
fn set_upstream_branch(repo: &Repository, remote_name: &str, branch_name: &str) -> Result<()> {
    let mut config = repo.config()?;
    let upstream_branch = format!("{remote_name}/{branch_name}");
    config.set_str(&format!("branch.{branch_name}.remote"), remote_name)?;
    config.set_str(
        &format!("branch.{branch_name}.merge"),
        &format!("refs/heads/{branch_name}"),
    )?;
    println!(
        "  {} Set upstream to {}",
        "✓".cyan(),
        upstream_branch.cyan()
    );
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
) -> Result<()> {
    if is_dry_run {
        dry_run.log_operation(Operation::GitPush {
            remote: remote_name.to_string(),
            branch: branch_name.to_string(),
            set_upstream,
        });
        return Ok(());
    }

    let remote = git_repo.inner().find_remote(remote_name)?;
    let remote_url = remote.url().unwrap_or("unknown");

    let head_before = git_repo.inner().head().ok().and_then(|head| head.target());
    let remote_branch_ref = format!("refs/remotes/{remote_name}/{branch_name}");
    let remote_ref_before = git_repo
        .inner()
        .find_reference(&remote_branch_ref)
        .ok()
        .and_then(|ref_| ref_.target());

    let start_time = std::time::Instant::now();
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

            if is_up_to_date {
                // Git-like output for up-to-date
                println!("Everything up-to-date");
            } else {
                display_push_success(
                    git_repo.inner(),
                    branch_name,
                    remote_name,
                    remote_url,
                    head_before,
                    remote_ref_before,
                );
            }
        }
        Err(err) => {
            if err.to_string().contains("timed out") {
                return Err(DotfilesError::Config(format!(
                    "Push operation timed out after {timeout_seconds} seconds"
                )));
            }

            let error_msg = format!("{err}");
            return Err(error_utils::git_operation_failed(
                "push",
                &git_repo.repo_path,
                &error_msg,
            ));
        }
    }

    if set_upstream {
        set_upstream_branch(git_repo.inner(), remote_name, branch_name)?;
    }

    Ok(())
}

/// Execute a git fetch operation with timeout.
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
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
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

/// Perform merge after fetch (fast-forward or regular merge).
/// Returns whether the merge was a fast-forward.
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

/// Check for merge conflicts and blocking untracked files.
fn check_merge_conflicts(repo: &Repository) -> Result<()> {
    let index = repo.index()?;
    if index.has_conflicts() {
        let conflicts: Vec<String> = index
            .conflicts()?
            .filter_map(|conflict| {
                conflict.ok().and_then(|conf| {
                    conf.our.map(|entry| {
                        std::str::from_utf8(&entry.path)
                            .map_or_else(|_| String::new(), ToString::to_string)
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

/// Calculate file statistics for a commit range (like git diff --stat).
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

    // Use diff stats for accurate file statistics
    let Ok(stats) = diff.stats() else {
        return (0, 0, 0);
    };

    let files_changed = stats.files_changed();
    let insertions = stats.insertions();
    let deletions = stats.deletions();

    (files_changed, insertions, deletions)
}

/// Parameters for displaying pull success message.
struct PullDisplayParams<'a> {
    /// Repository reference.
    repo: &'a Repository,
    /// Commit before pull.
    head_before: Option<git2::Commit<'a>>,
    /// Commit after pull.
    head_after: Option<git2::Commit<'a>>,
    /// Whether the merge was a fast-forward.
    was_fast_forward: bool,
}

/// Display pull success message with git-like output.
fn display_pull_success(params: PullDisplayParams<'_>) {
    use std::fmt::Write;

    if let (Some(head_before), Some(head_after)) = (params.head_before, params.head_after)
        && head_before.id() != head_after.id()
    {
        let before_short = head_before.id().to_string()[..7].to_string();
        let after_short = head_after.id().to_string()[..7].to_string();

        // Git-like output: "Updating abc1234..def5678"
        println!("Updating {before_short}..{after_short}");

        // Show fast-forward or merge info
        if params.was_fast_forward {
            println!("Fast-forward");
        } else {
            println!("Merge made by the 'recursive' strategy.");
        }

        // Calculate and show file statistics
        let (files_changed, insertions, deletions) =
            calculate_file_stats(params.repo, &head_before, &head_after);
        if files_changed > 0 {
            let mut stats = format!(
                " {files_changed} file{} changed",
                if files_changed == 1 { "" } else { "s" }
            );
            if insertions > 0 || deletions > 0 {
                let mut parts = Vec::new();
                if insertions > 0 {
                    parts.push(format!(
                        "{insertions} insertion{}",
                        if insertions == 1 { "" } else { "s" }
                    ));
                }
                if deletions > 0 {
                    parts.push(format!(
                        "{deletions} deletion{}",
                        if deletions == 1 { "" } else { "s" }
                    ));
                }
                let _ = write!(stats, ", {}", parts.join(", "));
            }
            println!("{stats}");
        }

        return;
    }

    // Already up-to-date
    println!("Already up-to-date.");
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
) -> Result<()> {
    if is_dry_run {
        dry_run.log_operation(Operation::GitPull {
            remote: remote_name.to_string(),
            branch: branch_name.to_string(),
        });
        return Ok(());
    }

    let remote = git_repo.inner().find_remote(remote_name)?;
    let _remote_url = remote.url().unwrap_or("unknown");

    let head_before = git_repo.inner().head().ok().and_then(|head| head.peel_to_commit().ok());
    let head_oid_before = head_before.as_ref().map(git2::Commit::id);

    let start_time = std::time::Instant::now();
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
    let _elapsed = start_time.elapsed();

    match fetch_result {
        Ok(()) => {
            let remote_branch_ref = format!("refs/remotes/{remote_name}/{branch_name}");
            let remote_branch =
                git_repo.inner().find_reference(&remote_branch_ref).map_err(|_| {
                    DotfilesError::Config(format!(
                        "Could not find remote branch {remote_name}/{branch_name} after fetch"
                    ))
                })?;

            let remote_commit = remote_branch.peel_to_commit()?;
            let remote_oid = remote_commit.id();

            if let Some(head_oid) = head_oid_before
                && head_oid == remote_oid
            {
                // Already up-to-date (no changes)
                return Ok(());
            }

            let was_fast_forward = perform_merge(
                git_repo.inner(),
                remote_name,
                branch_name,
                &remote_branch,
                &remote_commit,
            )?;

            let head_after =
                git_repo.inner().head().ok().and_then(|head| head.peel_to_commit().ok());
            let head_oid_after = head_after.as_ref().map(git2::Commit::id);

            if let (Some(oid_before), Some(oid_after)) = (head_oid_before, head_oid_after)
                && oid_before == oid_after
            {
                // Already up-to-date (no changes)
                return Ok(());
            }

            display_pull_success(PullDisplayParams {
                repo: git_repo.inner(),
                head_before,
                head_after,
                was_fast_forward,
            });
        }
        Err(err) => {
            if err.to_string().contains("timed out") {
                return Err(DotfilesError::Config(format!(
                    "Pull operation timed out after {timeout_seconds} seconds"
                )));
            }

            let error_msg = format!("{err}");
            return Err(error_utils::git_operation_failed(
                "pull",
                &git_repo.repo_path,
                &error_msg,
            ));
        }
    }

    Ok(())
}

/// Show git repository status (like `git status`).
///
/// # Errors
///
/// Returns an error if the repository status cannot be determined.
pub fn show_git_status(git_repo: &GitRepository) -> Result<()> {
    use git2::BranchType;

    // Get current branch
    let Ok(branch_name) = get_current_branch(git_repo) else {
        println!("{} Not on any branch", "⊘".yellow());
        return Ok(());
    };

    // Get branch reference
    let branch = git_repo.inner().find_branch(&branch_name, BranchType::Local)?;
    let upstream = branch.upstream();

    // Get HEAD commit
    let head = git_repo.inner().head()?;
    let head_commit = head.peel_to_commit()?;
    let head_oid = head_commit.id();
    let short_id = head_oid.to_string()[..7].to_string();

    // Check for uncommitted changes
    let changes = detect_changes(git_repo)?;
    let staged_count = changes.len();
    let has_uncommitted = !changes.is_empty();

    // Check ahead/behind and get upstream name if upstream exists
    let (ahead, behind, upstream_name) = if let Ok(upstream_branch) = upstream {
        let upstream_oid = upstream_branch.get().target().ok_or_else(|| {
            git2::Error::new(
                git2::ErrorCode::NotFound,
                git2::ErrorClass::Reference,
                "Could not find upstream target",
            )
        })?;

        let (ahead, behind) = git_repo.inner().graph_ahead_behind(head_oid, upstream_oid)?;
        let upstream_name = upstream_branch
            .get()
            .name()
            .map_or_else(|| "unknown".to_string(), ToString::to_string);
        (Some(ahead), Some(behind), Some(upstream_name))
    } else {
        (None, None, None)
    };

    // Display git status
    println!("\n{}", "Git Repository Status:".bold().cyan());
    println!("{}", "=".repeat(60).cyan());
    println!("  Branch: {}", branch_name.cyan());

    if let Some(ref upstream_name) = upstream_name {
        println!("  Upstream: {}", upstream_name.dimmed());
    }

    println!("  HEAD: {} {}", short_id.cyan(), {
        let message_raw = head_commit.message().unwrap_or("(no message)");
        message_raw.lines().next().unwrap_or("(no message)")
    });

    // Show ahead/behind
    if let (Some(ahead_count), Some(behind_count)) = (ahead, behind)
        && (ahead_count > 0 || behind_count > 0)
    {
        let mut status_parts = Vec::new();
        if ahead_count > 0 {
            status_parts.push(format!("{ahead_count} commit(s) ahead").green().to_string());
        }
        if behind_count > 0 {
            status_parts.push(format!("{behind_count} commit(s) behind").yellow().to_string());
        }
        if !status_parts.is_empty() {
            println!("  {}", status_parts.join(", "));
        }
    }

    // Show uncommitted changes
    if has_uncommitted {
        println!(
            "\n  {} {} uncommitted change(s):",
            "→".yellow(),
            staged_count
        );
        for change in &changes {
            let (icon, path) = match change {
                FileChange::Added(path) => ("+", path),
                FileChange::Modified(path) => ("M", path),
                FileChange::Deleted(path) => ("-", path),
            };
            let repo_root = git_repo.path().parent().ok_or_else(|| {
                DotfilesError::Config("Could not determine repository path".to_string())
            })?;
            if let Ok(relative) = path.strip_prefix(repo_root) {
                println!(
                    "    {} {}",
                    icon.green(),
                    relative.display().to_string().dimmed()
                );
            } else {
                println!(
                    "    {} {}",
                    icon.green(),
                    path.display().to_string().dimmed()
                );
            }
        }
    } else {
        println!("\n  {} Working tree clean", "✓".green());
    }

    println!();
    Ok(())
}
