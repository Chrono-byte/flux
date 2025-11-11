use crate::dry_run::{DryRun, Operation};
use crate::error::Result;
use crate::types::FileChange;
use colored::Colorize;
use git2::{Repository, Signature};
use std::path::Path;

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

    let signature = Signature::now("dotfiles-manager", "dotfiles-manager@localhost")?;

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

/// List all remotes in the repository
pub fn list_remotes(repo: &Repository) -> Result<()> {
    let remotes = repo.remotes()?;

    if remotes.is_empty() {
        println!("{} No remotes configured.", "⊘".yellow());
        return Ok(());
    }

    println!("\n{}", "Remotes:".bold().cyan());
    for name in remotes.iter() {
        if let Some(remote_name) = name
            && let Ok(remote) = repo.find_remote(remote_name)
        {
            let url = remote.url().unwrap_or("(invalid URL)");
            println!("  {} {}", remote_name.cyan(), url);
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

    // Construct refspec: refs/heads/branch:refs/heads/branch
    let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, branch_name);

    // Get or open the remote
    let mut remote = repo.find_remote(remote_name)?;

    // Create push options with callbacks for authentication
    let mut push_options = git2::PushOptions::new();
    let mut callbacks = git2::RemoteCallbacks::new();

    // Set up credentials callback
    callbacks.credentials(|_url, _user_from_url, _cred_type| {
        if let Ok(creds) = git2::Cred::ssh_key_from_agent("git") {
            return Ok(creds);
        }
        if let (Ok(user), Ok(pass)) = (std::env::var("GIT_USERNAME"), std::env::var("GIT_PASSWORD"))
            && let Ok(creds) = git2::Cred::userpass_plaintext(&user, &pass)
        {
            return Ok(creds);
        }
        Err(git2::Error::new(
            git2::ErrorCode::Auth,
            git2::ErrorClass::Reference,
            "No credentials available",
        ))
    });

    push_options.remote_callbacks(callbacks);

    // Push the branch
    remote.push(&[&refspec], Some(&mut push_options))?;

    println!(
        "{} Pushed {} to remote '{}' at {}",
        "✓".green(),
        branch_name,
        remote_name,
        remote.url().unwrap_or("unknown")
    );

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
