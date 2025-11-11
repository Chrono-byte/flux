use crate::error::{DotfilesError, Result};
use crate::types::FileChange;
use dialoguer::{Select, theme::ColorfulTheme};
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
    use dialoguer::Input;

    if changes.is_empty() {
        return Ok("Update dotfiles".to_string());
    }

    let mut messages = Vec::new();

    for change in changes {
        let change_desc = match change {
            FileChange::Added(path) => format!("File: {} (added)", path.display()),
            FileChange::Modified(path) => format!("File: {} (modified)", path.display()),
            FileChange::Deleted(path) => format!("File: {} (deleted)", path.display()),
        };

        let message: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt(format!(
                "{}\nEnter commit message for this change (or press Enter to skip):",
                change_desc
            ))
            .allow_empty(true)
            .interact_text()
            .map_err(|e| DotfilesError::Io(std::io::Error::other(e)))?;

        if !message.trim().is_empty() {
            messages.push(message);
        }
    }

    if messages.is_empty() {
        Ok("Update dotfiles".to_string())
    } else if messages.len() == 1 {
        Ok(messages[0].clone())
    } else {
        Ok(format!("Update dotfiles: {}", messages.join("; ")))
    }
}
