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

    for change in changes {
        match change {
            FileChange::Added(path) | FileChange::Modified(path) => {
                let repo_path = repo.path().parent().unwrap();
                if let Ok(relative) = path.strip_prefix(repo_path) {
                    index.add_path(relative)?;
                }
            }
            FileChange::Deleted(path) => {
                let repo_path = repo.path().parent().unwrap();
                if let Ok(relative) = path.strip_prefix(repo_path) {
                    index.remove_path(relative)?;
                }
            }
        }
    }

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
