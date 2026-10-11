use colored::Colorize;
use dialoguer::{Input, Select, theme::ColorfulTheme};
use flux_lib::types::FileChange;
use flux_lib::utils::error::{DotfilesError, Result};
use flux_lib::utils::prompt::{ConflictChoice, YesNoChoice};
use std::path::Path;

pub(super) fn prompt_conflict_choice(file_path: &Path) -> Result<ConflictChoice> {
  let options = vec![
    "Keep local version",
    "Use repo version",
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
    2 => Ok(ConflictChoice::ViewDiff),
    3 => Ok(ConflictChoice::Abort),
    _ => Err(DotfilesError::Cancelled),
  }
}

pub(super) fn prompt_yes_no(question: &str) -> Result<bool> {
  let options = vec!["Yes", "No"];

  let selection = Select::with_theme(&ColorfulTheme::default())
    .with_prompt(question)
    .items(&options)
    .default(0)
    .interact()
    .map_err(|error| DotfilesError::Io(std::io::Error::other(error)))?;

  let choice = match selection {
    0 => YesNoChoice::Yes,
    1 => YesNoChoice::No,
    _ => return Err(DotfilesError::Cancelled),
  };

  Ok(bool::from(choice))
}

pub(super) fn prompt_commit_message(changes: &[FileChange]) -> Result<String> {
  if changes.is_empty() {
    return Ok("Update dotfiles".to_string());
  }

  let mut change_summary = Vec::new();
  for change in changes {
    let change_desc = match change {
      FileChange::Added(path) => format!("  {} {}", "[+] Added:".green(), path.display()),
      FileChange::Modified(path) => format!("  {} {}", "[*] Modified:".yellow(), path.display()),
      FileChange::Deleted(path) => format!("  {} {}", "[-] Deleted:".red(), path.display()),
    };
    change_summary.push(change_desc);
  }
  let summary_text = change_summary.join("\n");

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

  Ok(if message.trim().is_empty() {
    "Update dotfiles".to_string()
  } else {
    message
  })
}
