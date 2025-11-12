use crate::types::FileChange;
use crate::utils::dry_run::{DryRun, Operation};
use crate::utils::error::{DotfilesError, Result};
use crate::utils::error_utils;
use colored::Colorize;
use git2::{CredentialType, FetchOptions, RemoteCallbacks, Repository, Signature};
use std::path::Path;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// Set up credential callbacks for git2 operations
/// Handles both SSH (via SSH agent) and HTTPS (via environment variables or system keyring) authentication
fn setup_credential_callbacks() -> RemoteCallbacks<'static> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username, allowed_types| {
        let username = username.unwrap_or("git");

        // For HTTPS authentication
        if allowed_types.contains(CredentialType::USER_PASS_PLAINTEXT) {
            // First try environment variables (explicit override)
            if let (Ok(user), Ok(pass)) =
                (std::env::var("GIT_USERNAME"), std::env::var("GIT_PASSWORD"))
                && let Ok(cred) = git2::Cred::userpass_plaintext(&user, &pass)
            {
                return Ok(cred);
            }

            // Try default credential helper (uses system keyring/credential manager)
            // This will use git's credential.helper config which may point to:
            // - macOS: osxkeychain
            // - Linux: libsecret, gnome-keyring, etc.
            // - Windows: wincred
            if let Ok(cred) = git2::Cred::default() {
                return Ok(cred);
            }
        }

        // For SSH authentication
        if allowed_types.contains(CredentialType::SSH_KEY) {
            // First try SSH agent (most common for SSH)
            if let Ok(cred) = git2::Cred::ssh_key_from_agent(username) {
                return Ok(cred);
            }

            // Try default credential helper (may have SSH keys configured)
            if let Ok(cred) = git2::Cred::default() {
                return Ok(cred);
            }
        }

        // For SSH, also try username-based credential (for custom SSH setups)
        if allowed_types.contains(CredentialType::USERNAME)
            && let Ok(cred) = git2::Cred::username(username)
        {
            return Ok(cred);
        }

        // Final fallback: default credential helper
        git2::Cred::default()
    });
    callbacks
}

/// Execute a git2 operation with a timeout
/// Spawns the operation in a thread and waits for completion or timeout
fn execute_with_timeout<F, T>(operation: F, timeout_seconds: u64) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = mpsc::channel();
    let timeout_duration = Duration::from_secs(timeout_seconds);

    // Spawn the operation in a thread
    thread::spawn(move || {
        let result = operation();
        let _ = tx.send(result);
    });

    // Wait for result or timeout
    match rx.recv_timeout(timeout_duration) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => Err(DotfilesError::Config(format!(
            "Operation timed out after {} seconds",
            timeout_seconds
        ))),
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(DotfilesError::Config(
            "Operation thread disconnected unexpectedly".to_string(),
        )),
    }
}

/// Get the user's git signature from their git config
/// This reads from the repository's config, which includes global git config
fn get_user_signature(repo: &Repository) -> Result<Signature<'_>> {
    let config = repo.config()?;

    let name = config.get_string("user.name").map_err(|e| {
        if e.code() == git2::ErrorCode::NotFound {
            DotfilesError::Config(
                "Git user.name is not configured. Please set it with: git config --global user.name \"Your Name\"".to_string()
            )
        } else {
            DotfilesError::Config(format!("Failed to read git user.name: {}", e))
        }
    })?;

    let email = config.get_string("user.email").map_err(|e| {
        if e.code() == git2::ErrorCode::NotFound {
            DotfilesError::Config(
                "Git user.email is not configured. Please set it with: git config --global user.email \"your.email@example.com\"".to_string()
            )
        } else {
            DotfilesError::Config(format!("Failed to read git user.email: {}", e))
        }
    })?;

    Signature::now(&name, &email)
        .map_err(|e| DotfilesError::Config(format!("Failed to create git signature: {}", e)))
}

pub fn init_repo(repo_path: &Path) -> Result<Repository> {
    let repo = if repo_path.join(".git").exists() {
        Repository::open(repo_path)?
    } else {
        Repository::init(repo_path)?
    };

    // Ensure the repository has a valid initial setup
    // Set the default branch to 'main' if not already set
    if repo.head().is_err() {
        // No HEAD exists yet (empty repository), create initial HEAD reference
        // Create a symbolic reference to refs/heads/main
        repo.set_head("refs/heads/main")?;
    }

    Ok(repo)
}

pub fn stage_changes(
    repo: &Repository,
    changes: &[FileChange],
    dry_run: &mut DryRun,
    is_dry_run: bool,
) -> Result<()> {
    if is_dry_run {
        let files: Vec<_> = changes
            .iter()
            .map(|c| match c {
                FileChange::Added(p) | FileChange::Modified(p) | FileChange::Deleted(p) => {
                    p.clone()
                }
            })
            .collect();
        dry_run.log_operation(Operation::GitStage { files });
        return Ok(());
    }

    let mut index = repo.index()?;
    let repo_path = repo.path().parent().unwrap();

    for change in changes {
        match change {
            FileChange::Added(path) | FileChange::Modified(path) => {
                // If it's a directory, recursively add all files in it
                if path.is_dir() {
                    for entry in walkdir::WalkDir::new(path)
                        .into_iter()
                        .filter_map(|e| e.ok())
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
                    for entry in walkdir::WalkDir::new(path)
                        .into_iter()
                        .filter_map(|e| e.ok())
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

pub fn commit_changes(
    repo: &Repository,
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
    let mut index = repo.index()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    // Use the user's git config for commit signature
    let signature = get_user_signature(repo)?;

    let head = repo.head();
    let parent_commit = if let Ok(head) = head {
        Some(head.peel_to_commit()?)
    } else {
        None
    };

    let parents: Vec<&git2::Commit> = parent_commit.iter().collect();

    repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        commit_message,
        &tree,
        &parents,
    )?;

    // After committing, refresh the index to ensure it's in sync with the committed tree
    // This prevents old staged entries from showing up as changed on the next status check
    let mut new_index = repo.index()?;
    new_index.read_tree(&tree)?;
    new_index.write()?;

    println!("{} Committed changes: {}", "✓".green(), commit_message);
    Ok(())
}

pub fn detect_changes(repo: &Repository) -> Result<Vec<FileChange>> {
    let mut changes = Vec::new();
    let mut status_options = git2::StatusOptions::new();
    status_options.include_untracked(true);
    status_options.include_ignored(false);

    let statuses = repo.statuses(Some(&mut status_options))?;

    for entry in statuses.iter() {
        let path = entry.path().unwrap();
        let status = entry.status();

        let repo_path = repo.path().parent().unwrap().join(path);

        if status.is_index_new() || status.is_wt_new() {
            changes.push(FileChange::Added(repo_path));
        } else if status.is_index_modified() || status.is_wt_modified() {
            changes.push(FileChange::Modified(repo_path));
        } else if status.is_index_deleted() || status.is_wt_deleted() {
            changes.push(FileChange::Deleted(repo_path));
        }
    }

    Ok(changes)
}

/// Get the current branch name (shorthand of HEAD)
pub fn get_current_branch(repo: &Repository) -> Result<String> {
    let head = repo.head()?;
    let shorthand = head.shorthand().ok_or_else(|| {
        git2::Error::new(
            git2::ErrorCode::Invalid,
            git2::ErrorClass::Reference,
            "Could not determine current branch",
        )
    })?;
    Ok(shorthand.to_string())
}

/// Add a remote to the repository
pub fn add_remote(
    repo: &Repository,
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

    repo.remote(name, url)?;
    println!("{} Added remote '{}': {}", "✓".green(), name, url);
    Ok(())
}

/// Remove a remote from the repository
pub fn remove_remote(
    repo: &Repository,
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

    repo.remote_delete(name)?;
    println!("{} Removed remote '{}'", "✓".green(), name);
    Ok(())
}

/// Set or update a remote URL
pub fn set_remote_url(
    repo: &Repository,
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

    repo.remote_set_url(name, url)?;
    println!("{} Set URL for remote '{}': {}", "✓".green(), name, url);
    Ok(())
}

/// List all remotes in the repository (like `git remote -v`)
pub fn list_remotes(repo: &Repository) -> Result<()> {
    let remotes = repo.remotes()?;

    if remotes.is_empty() {
        println!("{} No remotes configured.", "⊘".yellow());
        return Ok(());
    }

    println!();
    for name in remotes.iter() {
        if let Some(remote_name) = name
            && let Ok(remote) = repo.find_remote(remote_name)
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

/// Push to a remote repository
pub fn push_to_remote(
    repo: &Repository,
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

    let repo_path = repo
        .path()
        .parent()
        .ok_or_else(|| DotfilesError::Config("Could not determine repository path".to_string()))?;

    // Get remote URL for display
    let remote = repo.find_remote(remote_name)?;
    let remote_url = remote.url().unwrap_or("unknown");

    // Get current HEAD before push to detect if anything was pushed
    let head_before = repo.head().ok().and_then(|h| h.target());
    let remote_branch_ref = format!("refs/remotes/{}/{}", remote_name, branch_name);

    // Check if remote tracking branch exists and what it points to
    let remote_ref_before = repo
        .find_reference(&remote_branch_ref)
        .ok()
        .and_then(|r| r.target());

    // Set up push options with credential callbacks (not used directly, but kept for consistency)
    let _push_options = git2::PushOptions::new();
    let _callbacks = setup_credential_callbacks();

    // Prepare refspec
    let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);

    // Execute push with timeout
    // Clone necessary data to move into thread (Repository is not Send)
    let repo_path_clone = repo_path.to_path_buf();
    let remote_name_clone = remote_name.to_string();
    let refspec_clone = refspec.clone();

    let start_time = std::time::Instant::now();
    let push_result = execute_with_timeout(
        move || -> Result<()> {
            // Open repository in thread (git2 operations are thread-safe for different Repository instances)
            let repo_in_thread = Repository::open(&repo_path_clone)?;
            let mut remote = repo_in_thread.find_remote(&remote_name_clone)?;
            let mut push_options_in_thread = git2::PushOptions::new();
            let callbacks_in_thread = setup_credential_callbacks();
            push_options_in_thread.remote_callbacks(callbacks_in_thread);
            remote.push(&[&refspec_clone], Some(&mut push_options_in_thread))?;
            Ok(())
        },
        timeout_seconds,
    );

    let elapsed = start_time.elapsed();

    // Handle push result
    match push_result {
        Ok(()) => {
            // Check if anything was actually pushed by comparing refs
            let remote_ref_after = repo
                .find_reference(&remote_branch_ref)
                .ok()
                .and_then(|r| r.target());
            let is_up_to_date =
                if let (Some(before), Some(after)) = (remote_ref_before, remote_ref_after) {
                    before == after && head_before == Some(before)
                } else if remote_ref_before.is_none() {
                    // Remote branch didn't exist before, so we pushed something
                    false
                } else {
                    // Can't determine, assume something was pushed
                    false
                };

            if is_up_to_date {
                println!(
                    "{} Everything up-to-date with {}/{}",
                    "✓".green(),
                    remote_name,
                    branch_name
                );
            } else {
                // Show what was pushed
                println!(
                    "{} Pushed {} to remote '{}' at {} (took {:.2}s)",
                    "✓".green(),
                    branch_name,
                    remote_name,
                    remote_url,
                    elapsed.as_secs_f64()
                );

                // Show commit info
                if let Ok(head) = repo.head()
                    && let Ok(commit) = head.peel_to_commit()
                {
                    let message = commit
                        .message()
                        .unwrap_or("(no message)")
                        .lines()
                        .next()
                        .unwrap_or("(no message)");
                    let short_id = commit.id().to_string()[..7].to_string();
                    println!(
                        "  {} {} {} {}",
                        "→".cyan(),
                        short_id.cyan(),
                        message,
                        commit.author().name().unwrap_or("unknown").dimmed()
                    );
                }
            }
        }
        Err(e) => {
            // Check if it's a timeout error
            if e.to_string().contains("timed out") {
                return Err(DotfilesError::Config(format!(
                    "Push operation timed out after {} seconds",
                    timeout_seconds
                )));
            }

            // Convert git2 error to user-friendly error
            let error_msg = format!("{}", e);
            return Err(error_utils::git_operation_failed(
                "push", repo_path, &error_msg,
            ));
        }
    }

    // Set upstream if requested
    if set_upstream {
        let mut config = repo.config()?;
        let upstream_branch = format!("{}/{}", remote_name, branch_name);
        config.set_str(&format!("branch.{}.remote", branch_name), remote_name)?;
        config.set_str(
            &format!("branch.{}.merge", branch_name),
            &format!("refs/heads/{}", branch_name),
        )?;
        println!(
            "  {} Set upstream to {}",
            "✓".cyan(),
            upstream_branch.cyan()
        );
    }

    Ok(())
}

/// Pull from a remote repository
pub fn pull_from_remote(
    repo: &Repository,
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

    let repo_path = repo
        .path()
        .parent()
        .ok_or_else(|| DotfilesError::Config("Could not determine repository path".to_string()))?;

    // Get remote URL for display
    let remote = repo.find_remote(remote_name)?;
    let remote_url = remote.url().unwrap_or("unknown");

    // Get current HEAD commit before pulling to detect changes
    let head_before = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
    let head_oid_before = head_before.as_ref().map(|c| c.id());

    // Check for untracked files that might conflict with merge
    let _index = repo.index()?;
    let mut untracked_files = Vec::new();
    let statuses = repo.statuses(Some(
        &mut git2::StatusOptions::new().include_untracked(true),
    ))?;
    for entry in statuses.iter() {
        if entry.status().is_wt_new()
            && let Some(path) = entry.path()
        {
            untracked_files.push(path.to_string());
        }
    }

    // Set up fetch options with credential callbacks (not used directly, but kept for consistency)
    let _fetch_options = FetchOptions::new();
    let _callbacks = setup_credential_callbacks();

    // Fetch from remote with timeout
    // Clone necessary data to move into thread (Repository is not Send)
    let repo_path_clone = repo_path.to_path_buf();
    let remote_name_clone = remote_name.to_string();
    let branch_name_clone = branch_name.to_string();

    let start_time = std::time::Instant::now();
    let fetch_result = execute_with_timeout(
        move || -> Result<()> {
            // Open repository in thread (git2 operations are thread-safe for different Repository instances)
            let repo_in_thread = Repository::open(&repo_path_clone)?;
            let mut remote = repo_in_thread.find_remote(&remote_name_clone)?;
            let mut fetch_options_in_thread = FetchOptions::new();
            let callbacks_in_thread = setup_credential_callbacks();
            fetch_options_in_thread.remote_callbacks(callbacks_in_thread);
            let refspec_in_thread = format!(
                "refs/heads/{}:refs/remotes/{}/{}",
                branch_name_clone, remote_name_clone, branch_name_clone
            );
            remote.fetch(
                &[&refspec_in_thread],
                Some(&mut fetch_options_in_thread),
                None,
            )?;
            Ok(())
        },
        timeout_seconds,
    );

    let elapsed = start_time.elapsed();

    // Handle fetch result
    match fetch_result {
        Ok(()) => {
            // Fetch succeeded, now merge
            let remote_branch_ref = format!("refs/remotes/{}/{}", remote_name, branch_name);
            let remote_branch = match repo.find_reference(&remote_branch_ref) {
                Ok(ref_) => ref_,
                Err(_) => {
                    return Err(DotfilesError::Config(format!(
                        "Could not find remote branch {}/{} after fetch",
                        remote_name, branch_name
                    )));
                }
            };

            let remote_commit = remote_branch.peel_to_commit()?;
            let remote_oid = remote_commit.id();

            // Check if already up to date
            if let Some(head_oid) = head_oid_before
                && head_oid == remote_oid
            {
                println!(
                    "{} Already up to date with {}/{}",
                    "✓".green(),
                    remote_name,
                    branch_name
                );
                return Ok(());
            }

            // Check for untracked files that would be overwritten
            // This is a simplified check - in practice, git checks more carefully
            if !untracked_files.is_empty() {
                // Try to merge and see if it fails due to untracked files
                // We'll detect this in the merge error handling
            }

            // Perform merge
            let annotated_commit = repo.reference_to_annotated_commit(&remote_branch)?;
            let (analysis, _) = repo.merge_analysis(&[&annotated_commit])?;

            if analysis.is_up_to_date() {
                println!(
                    "{} Already up to date with {}/{}",
                    "✓".green(),
                    remote_name,
                    branch_name
                );
                return Ok(());
            }

            if analysis.is_fast_forward() {
                // Fast-forward merge
                let mut ref_ = repo.find_reference(&format!("refs/heads/{}", branch_name))?;
                ref_.set_target(remote_oid, "Fast-forward")?;
                repo.set_head(&format!("refs/heads/{}", branch_name))?;
                repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;
            } else {
                // Regular merge
                let signature = get_user_signature(repo)?;
                repo.merge(&[&annotated_commit], None, None)?;

                // Check for conflicts
                let mut index = repo.index()?;
                if index.has_conflicts() {
                    // Extract conflicting files
                    let conflicts: Vec<String> = index
                        .conflicts()?
                        .filter_map(|conflict| {
                            conflict.ok().and_then(|c| {
                                c.our.map(|entry| {
                                    std::str::from_utf8(&entry.path).unwrap_or("").to_string()
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

                // Check for untracked files that would be overwritten
                // This happens when merge tries to create a file that exists as untracked
                let statuses = repo.statuses(Some(
                    &mut git2::StatusOptions::new().include_untracked(true),
                ))?;
                let blocking_files: Vec<String> = statuses
                    .iter()
                    .filter_map(|entry| {
                        if entry.status().is_wt_new() {
                            entry.path().map(|p| p.to_string())
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

                // Create merge commit
                let tree_id = index.write_tree()?;
                let tree = repo.find_tree(tree_id)?;
                let head = repo.head()?.peel_to_commit()?;
                repo.commit(
                    Some("HEAD"),
                    &signature,
                    &signature,
                    &format!("Merge {}/{}", remote_name, branch_name),
                    &tree,
                    &[&head, &remote_commit],
                )?;
            }

            // Check if we're already up to date after merge
            let head_after = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
            let head_oid_after = head_after.as_ref().map(|c| c.id());

            if let (Some(oid_before), Some(oid_after)) = (head_oid_before, head_oid_after) {
                if oid_before == oid_after {
                    println!(
                        "{} Already up to date with {}/{}",
                        "✓".green(),
                        remote_name,
                        branch_name
                    );
                    return Ok(());
                }

                // Show what was pulled
                if let (Some(head_before), Some(head_after)) = (head_before, head_after) {
                    if head_before.id() != head_after.id() {
                        // Find commits between old and new HEAD
                        let mut revwalk = repo.revwalk()?;
                        revwalk.push(head_after.id())?;
                        revwalk.hide(head_before.id())?;
                        revwalk.set_sorting(git2::Sort::TIME | git2::Sort::TOPOLOGICAL)?;

                        let commits: Vec<_> = revwalk
                            .take(10) // Limit to 10 most recent commits
                            .filter_map(|oid| oid.ok())
                            .filter_map(|oid| repo.find_commit(oid).ok())
                            .collect();

                        if !commits.is_empty() {
                            println!(
                                "{} Pulled {} from remote '{}' at {} (took {:.2}s)",
                                "✓".green(),
                                branch_name,
                                remote_name,
                                remote_url,
                                elapsed.as_secs_f64()
                            );
                            println!("  {} {} new commit(s):", "→".cyan(), commits.len());
                            for commit in commits.iter().rev() {
                                let message = commit
                                    .message()
                                    .unwrap_or("(no message)")
                                    .lines()
                                    .next()
                                    .unwrap_or("(no message)");
                                let short_id = commit.id().to_string()[..7].to_string();
                                println!(
                                    "    {} {} {}",
                                    short_id.cyan(),
                                    message,
                                    commit.author().name().unwrap_or("unknown").dimmed()
                                );
                            }
                        } else {
                            // Fallback: just show the pull succeeded
                            println!(
                                "{} Pulled {} from remote '{}' at {} (took {:.2}s)",
                                "✓".green(),
                                branch_name,
                                remote_name,
                                remote_url,
                                elapsed.as_secs_f64()
                            );
                        }
                    }
                } else {
                    // Fallback: show basic success message
                    println!(
                        "{} Pulled {} from remote '{}' at {} (took {:.2}s)",
                        "✓".green(),
                        branch_name,
                        remote_name,
                        remote_url,
                        elapsed.as_secs_f64()
                    );
                }
            }
        }
        Err(e) => {
            // Check if it's a timeout error
            if e.to_string().contains("timed out") {
                return Err(DotfilesError::Config(format!(
                    "Pull operation timed out after {} seconds",
                    timeout_seconds
                )));
            }

            // Convert git2 error to user-friendly error
            let error_msg = format!("{}", e);
            return Err(error_utils::git_operation_failed(
                "pull", repo_path, &error_msg,
            ));
        }
    }

    Ok(())
}

/// Show git repository status (like `git status`)
pub fn show_git_status(repo: &Repository) -> Result<()> {
    use git2::BranchType;

    // Get current branch
    let branch_name = match get_current_branch(repo) {
        Ok(name) => name,
        Err(_) => {
            println!("{} Not on any branch", "⊘".yellow());
            return Ok(());
        }
    };

    // Get branch reference
    let branch = repo.find_branch(&branch_name, BranchType::Local)?;
    let upstream = branch.upstream();

    // Get HEAD commit
    let head = repo.head()?;
    let head_commit = head.peel_to_commit()?;
    let head_oid = head_commit.id();
    let short_id = head_oid.to_string()[..7].to_string();

    // Check for uncommitted changes
    let changes = detect_changes(repo)?;
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

        let (ahead, behind) = repo.graph_ahead_behind(head_oid, upstream_oid)?;
        let upstream_name = upstream_branch
            .get()
            .name()
            .unwrap_or("unknown")
            .to_string();
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
        head_commit
            .message()
            .unwrap_or("(no message)")
            .lines()
            .next()
            .unwrap_or("(no message)")
    });

    // Show ahead/behind
    if let (Some(a), Some(b)) = (ahead, behind)
        && (a > 0 || b > 0)
    {
        let mut status_parts = Vec::new();
        if a > 0 {
            status_parts.push(format!("{} commit(s) ahead", a).green().to_string());
        }
        if b > 0 {
            status_parts.push(format!("{} commit(s) behind", b).yellow().to_string());
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
                FileChange::Added(p) => ("+", p),
                FileChange::Modified(p) => ("M", p),
                FileChange::Deleted(p) => ("-", p),
            };
            let repo_path = repo.path().parent().unwrap();
            if let Ok(relative) = path.strip_prefix(repo_path) {
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
