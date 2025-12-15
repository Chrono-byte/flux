use crate::config::Config;
use crate::services::transactions::{BackupRecord, Transaction};
use crate::utils::error::{DotfilesError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// A single backup entry in a generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationBackup {
    #[serde(with = "pathbuf_as_string")]
    pub backup_path: PathBuf,
    #[serde(with = "pathbuf_as_string")]
    pub target_path: PathBuf,
}

mod pathbuf_as_string {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::path::{Path, PathBuf};

    pub fn serialize<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&path.to_string_lossy())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PathBuf, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(PathBuf::from(s))
    }
}

/// A recorded apply "generation" that can be inspected or rolled back.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Generation {
    pub id: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    pub profile: Option<String>,
    pub description: Option<String>,
    pub backups: Vec<GenerationBackup>,
}

fn generations_dir(config: &Config) -> Result<PathBuf> {
    let base = config.get_backup_dir()?;
    Ok(base.join("generations"))
}

fn generation_path(config: &Config, id: &str) -> Result<PathBuf> {
    Ok(generations_dir(config)?.join(format!("{id}.json")))
}

/// Record a completed transaction as a generation on disk.
pub fn record_generation(config: &Config, transaction: &Transaction) -> Result<()> {
    let dir = generations_dir(config)?;
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }

    // Extract metadata
    let timestamp = if let Some(ts) = transaction.metadata.get("timestamp") {
        ts.parse::<DateTime<Utc>>().map_err(|e| {
            DotfilesError::Config(format!("Invalid transaction timestamp '{}': {e}", ts))
        })?
    } else {
        Utc::now()
    };

    let profile = transaction.metadata.get("profile").cloned();
    let description = transaction.metadata.get("description").cloned();

    let backups = transaction
        .backups
        .iter()
        .map(|b: &BackupRecord| GenerationBackup {
            backup_path: b.backup_path.clone(),
            target_path: b.target_path.clone(),
        })
        .collect();

    let generation = Generation {
        id: transaction.id.clone(),
        timestamp,
        profile,
        description,
        backups,
    };

    let path = generation_path(config, &generation.id)?;
    let json = serde_json::to_string_pretty(&generation).map_err(|e| {
        DotfilesError::Config(format!("Failed to serialize generation metadata: {e}"))
    })?;
    fs::write(&path, json)?;

    Ok(())
}

/// Load a generation by id.
pub fn load_generation(config: &Config, id: &str) -> Result<Generation> {
    let path = generation_path(config, id)?;
    if !path.exists() {
        return Err(DotfilesError::Path(format!(
            "Generation '{}' not found at {}",
            id,
            path.display()
        )));
    }

    let data = fs::read_to_string(&path)?;
    let generation: Generation = serde_json::from_str(&data)
        .map_err(|e| DotfilesError::Config(format!("Failed to parse generation metadata: {e}")))?;
    Ok(generation)
}

/// List generations, newest first, optionally limited.
pub fn list_generations(config: &Config, limit: Option<usize>) -> Result<Vec<Generation>> {
    let dir = generations_dir(config)?;
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut gens = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        let is_json = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|ext| ext == "json");
        if path.is_file() && matches!(is_json, Some(true)) {
            let data = fs::read_to_string(&path)?;
            if let Ok(generation) = serde_json::from_str::<Generation>(&data) {
                gens.push(generation);
            }
        }
    }

    // Newest first
    gens.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    if let Some(max) = limit {
        gens.truncate(max);
    }

    Ok(gens)
}

/// Restore files from a generation's backups.
pub fn restore_generation(_config: &Config, generation: &Generation, dry_run: bool) -> Result<()> {
    let mut tracker = crate::utils::dry_run::DryRun::default();
    let mut fs_manager = crate::file_manager::FileSystemManager::new(&mut tracker, dry_run);

    if generation.backups.is_empty() {
        println!("No backups recorded for generation {}", generation.id);
        return Ok(());
    }

    for backup in &generation.backups {
        let backup_path = &backup.backup_path;
        let target_path = &backup.target_path;

        if !backup_path.exists() {
            println!(
                "  Skipping {} -> {} (backup missing)",
                backup_path.display(),
                target_path.display()
            );
            continue;
        }

        if let Some(parent) = target_path.parent() {
            fs_manager.create_dir_all(parent)?;
        }

        if backup_path.is_dir() {
            fs_manager.copy_dir_all(backup_path, target_path)?;
        } else {
            fs_manager.copy(backup_path, target_path)?;
        }
    }

    if dry_run {
        tracker.display_summary();
    } else {
        println!(
            "✓ Restored {} path(s) from generation {}",
            generation.backups.len(),
            generation.id
        );
    }

    Ok(())
}
