use crate::types::FileChange;
use crate::utils::error::{DotfilesError, Result};
use colored::Colorize;
use dialoguer::{Input, Select, theme::ColorfulTheme};
use std::path::Path;

/// User choice for yes/no prompts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YesNoChoice {
    /// User selected yes
    Yes,
    /// User selected no
    No,
}

impl From<YesNoChoice> for bool {
    fn from(choice: YesNoChoice) -> bool {
        matches!(choice, YesNoChoice::Yes)
    }
}

/// User choice for conflict resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
    /// Keep local version
    KeepLocal,
    /// Use repo version
    UseRepo,
    /// View both versions first
    ViewDiff,
    /// Abort the operation
    Abort,
    /// Backup local and use repo
    BackupAndUseRepo,
}

/// Prompt the user for a conflict resolution using the modern `ConflictChoice` enum.
///
/// # Errors
///
/// Returns an error if the user cannot be prompted.
pub fn prompt_conflict_choice(file_path: &Path) -> Result<ConflictChoice> {
    let options = vec![
        "Keep local version",
        "Use repo version",
        "Backup local and use repo",
        "View diff",
        "Abort",
    ];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "Conflict detected at: {}\nHow would you like to proceed?",
            file_path.display()
        ))
        .items(&options)
        .default(0)
        .interact()
        .map_err(|error| DotfilesError::Io(std::io::Error::other(error)))?;

    match selection {
        0 => Ok(ConflictChoice::KeepLocal),
        1 => Ok(ConflictChoice::UseRepo),
        2 => Ok(ConflictChoice::BackupAndUseRepo),
        3 => Ok(ConflictChoice::ViewDiff),
        4 => Ok(ConflictChoice::Abort),
        _ => Err(DotfilesError::Cancelled),
    }
}

/// Prompt the user for a yes/no answer using the modern `YesNoChoice` enum.
///
/// # Errors
///
/// Returns an error if the prompt interaction fails.
pub fn prompt_yes_no_choice(question: &str) -> Result<YesNoChoice> {
    let options = vec!["Yes", "No"];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(question)
        .items(&options)
        .default(0)
        .interact()
        .map_err(|error| DotfilesError::Io(std::io::Error::other(error)))?;

    match selection {
        0 => Ok(YesNoChoice::Yes),
        1 => Ok(YesNoChoice::No),
        _ => Err(DotfilesError::Cancelled),
    }
}

/// Prompt the user for a yes/no answer (legacy API).
///
/// # Errors
///
/// Returns an error if the prompt interaction fails.
pub fn prompt_yes_no(question: &str) -> Result<bool> {
    let choice = prompt_yes_no_choice(question)?;
    Ok(choice.into())
}

/// Prompt the user for a commit message.
///
/// # Errors
///
/// Returns an error if the prompt interaction fails.
pub fn prompt_commit_message(changes: &[FileChange]) -> Result<String> {
    if changes.is_empty() {
        return Ok("Update dotfiles".to_string());
    }

    // --- 1. Build a formatted summary of all changes ---
    let mut change_summary = Vec::new();
    for change in changes {
        let change_desc = match change {
            FileChange::Added(path) => {
                format!("  {} {}", "[+] Added:".green(), path.display())
            }
            FileChange::Modified(path) => {
                format!("  {} {}", "[*] Modified:".yellow(), path.display())
            }
            FileChange::Deleted(path) => {
                format!("  {} {}", "[-] Deleted:".red(), path.display())
            }
        };
        change_summary.push(change_desc);
    }
    let summary_text = change_summary.join("\n");

    // --- 2. Create a single prompt with the summary ---
    let prompt_text = format!(
        "{}\n{}\n\nEnter commit message (or press Enter for 'Update dotfiles'):",
        "The following changes will be committed:".bold(),
        summary_text
    );

    let message: String = Input::with_theme(&ColorfulTheme::default())
        .with_prompt(prompt_text)
        .allow_empty(true)
        .interact_text()
        .map_err(|error| DotfilesError::Io(std::io::Error::other(error)))?;

    // --- 3. Handle the response ---
    Ok(if message.trim().is_empty() {
        "Update dotfiles".to_string()
    } else {
        message
    })
}
