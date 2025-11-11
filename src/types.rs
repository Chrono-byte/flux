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

// ==================== Package Management Types ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageSpec {
    /// Package name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    
    /// Version constraint (e.g., "latest", "1.9.0", "~1.9")
    #[serde(default = "default_latest")]
    pub version: String,
    
    /// Optional: minimum version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_version: Option<String>,
    
    /// Optional: maximum version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_version: Option<String>,
    
    /// Description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_latest() -> String {
    "latest".to_string()
}

impl PackageSpec {
    pub fn new(name: String) -> Self {
        Self {
            name: Some(name),
            version: "latest".to_string(),
            min_version: None,
            max_version: None,
            description: None,
        }
    }
    
    pub fn with_version(name: String, version: String) -> Self {
        Self {
            name: Some(name),
            version,
            min_version: None,
            max_version: None,
            description: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub available_version: String,
    pub description: String,
    pub source: PackageSource,
}

#[derive(Debug, Clone)]
pub enum PackageSource {
    Fedora,
    Updates,
    RPMFusion,
    Copr(String),
    Custom(String),
}

impl std::fmt::Display for PackageSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackageSource::Fedora => write!(f, "fedora"),
            PackageSource::Updates => write!(f, "updates"),
            PackageSource::RPMFusion => write!(f, "rpmfusion"),
            PackageSource::Copr(name) => write!(f, "copr:{}", name),
            PackageSource::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

// ==================== Service Management Types ====================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSpec {
    /// Service name (derived from key if not specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    
    /// Package that provides this service (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    
    /// Should be enabled/started at boot
    #[serde(default)]
    pub enabled: bool,
    
    /// Should be running now
    #[serde(skip_serializing_if = "Option::is_none")]
    pub running: Option<bool>,
    
    /// Autostart on user login
    #[serde(default)]
    pub autostart: bool,
}

impl ServiceSpec {
    pub fn new(name: String) -> Self {
        Self {
            name: Some(name),
            package: None,
            enabled: false,
            running: None,
            autostart: false,
        }
    }
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
