use crate::types::FileChange;

/// User choice for yes/no prompts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YesNoChoice {
  /// User selected yes.
  Yes,
  /// User selected no.
  No,
}

impl From<YesNoChoice> for bool {
  fn from(choice: YesNoChoice) -> bool {
    matches!(choice, YesNoChoice::Yes)
  }
}

/// User choice for conflict resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictChoice {
  /// Keep local version.
  KeepLocal,
  /// Use repo version.
  UseRepo,
  /// View both versions first.
  ViewDiff,
  /// Abort the operation.
  Abort,
}

/// Prompt payload for commit-message collection.
#[derive(Debug, Clone)]
pub struct CommitPrompt {
  /// Changes that will be committed.
  pub changes: Vec<FileChange>,
}
