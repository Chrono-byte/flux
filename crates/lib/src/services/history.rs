use crate::utils::error::{DotfilesError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, Write};
use std::path::PathBuf;

/// A single history event entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEvent {
  /// Timestamp when the event occurred.
  pub timestamp: DateTime<Utc>,
  /// Command that was executed.
  pub command: String,
  /// Profile name used, if any.
  pub profile: Option<String>,
  /// Whether this was a dry-run operation.
  pub dry_run: bool,
  /// Transaction ID associated with this event, if any.
  pub transaction_id: Option<String>,
  /// Optional summary of the operation.
  pub summary: Option<String>,
}

/// Get the path to the history file, creating the directory if needed.
fn history_file() -> Result<PathBuf> {
  let data_dir = dirs::data_local_dir()
    .ok_or_else(|| DotfilesError::Config("Could not find data directory".to_string()))?;
  let flux_dir = data_dir.join("flux");
  if !flux_dir.exists() {
    fs::create_dir_all(&flux_dir)?;
  }
  Ok(flux_dir.join("history.jsonl"))
}

/// Record a history event to the log file.
///
/// # Errors
///
/// Returns an error if the history file cannot be created, opened, or written to.
pub fn record_event(event: &HistoryEvent) -> Result<()> {
  let path = history_file()?;
  let mut file = fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(&path)?;

  let json = serde_json::to_string(&event).map_err(|error| {
    DotfilesError::Config(format!("Failed to serialize history event: {error}"))
  })?;
  writeln!(file, "{json}")?;

  Ok(())
}

/// Read history events, newest first, optionally limited.
///
/// # Errors
///
/// Returns an error if the history file cannot be read or parsed.
pub fn read_events(limit: Option<usize>) -> Result<Vec<HistoryEvent>> {
  let path = history_file()?;
  if !path.exists() {
    return Ok(Vec::new());
  }

  let file = fs::File::open(&path)?;
  let reader = std::io::BufReader::new(file);
  let mut events = Vec::new();

  for line in reader.lines() {
    let line = line?;
    if let Ok(event) = serde_json::from_str::<HistoryEvent>(&line) {
      events.push(event);
    }
  }

  // Newest first
  events.sort_by_key(|event| std::cmp::Reverse(event.timestamp));

  if let Some(max) = limit {
    events.truncate(max);
  }

  Ok(events)
}
