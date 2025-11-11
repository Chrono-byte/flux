use crate::types::FileChange;
use crate::utils::error::{DotfilesError, Result};
use colored::Colorize;
use dialoguer::{Input, Select, theme::ColorfulTheme};
use std::path::Path;

pub enum ConflictResolution {
    BackupAndReplace,
    Skip,
    ViewDiff,
    Cancel,
}

pub fn prompt_conflict(file_path: &Path) -> Result<ConflictResolution> {
    let options = vec!["Backup and replace", "Skip", "View diff", "Cancel"];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!(
            "Conflict detected at: {}\nHow would you like to proceed?",
            file_path.display()
        ))
        .items(&options)
        .default(0)
        .interact()
        .map_err(|e| DotfilesError::Io(std::io::Error::other(e)))?;

    match selection {
        0 => Ok(ConflictResolution::BackupAndReplace),
        1 => Ok(ConflictResolution::Skip),
        2 => Ok(ConflictResolution::ViewDiff),
        3 => Ok(ConflictResolution::Cancel),
        _ => Err(DotfilesError::Cancelled),
    }
}

pub fn prompt_yes_no(question: &str) -> Result<bool> {
    let options = vec!["Yes", "No"];

    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(question)
        .items(&options)
        .default(0)
        .interact()
        .map_err(|e| DotfilesError::Io(std::io::Error::other(e)))?;

    Ok(selection == 0)
}

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
        .map_err(|e| DotfilesError::Io(std::io::Error::other(e)))?;

    // --- 3. Handle the response ---
    Ok(if message.trim().is_empty() {
        "Update dotfiles".to_string()
    } else {
        message
    })
}
