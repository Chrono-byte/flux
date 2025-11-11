use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub repo: String,
    pub dest: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChange {
    Added(PathBuf),
    Modified(PathBuf),
    Deleted(PathBuf),
}

// ==================== Environment Types ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentSpec {
    /// Environment variables to set
    #[serde(default)]
    pub variables: HashMap<String, String>,

    /// Shell to use
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shell: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SymlinkResolution {
    Auto,
    Relative,
    Absolute,
    Follow,
    Replace,
}

impl std::str::FromStr for SymlinkResolution {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "auto" => Ok(SymlinkResolution::Auto),
            "relative" => Ok(SymlinkResolution::Relative),
            "absolute" => Ok(SymlinkResolution::Absolute),
            "follow" => Ok(SymlinkResolution::Follow),
            "replace" => Ok(SymlinkResolution::Replace),
            _ => Err(format!("Invalid symlink resolution: {}", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TrackedFile {
    pub tool: String,
    pub repo_path: PathBuf,
    pub dest_path: PathBuf,
    pub profile: Option<String>,
}
