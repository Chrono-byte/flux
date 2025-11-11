use crate::types::{InstalledPackage, PackageInfo, PackageSource};
use crate::utils::error::{DotfilesError, Result};
use std::process::Command;

/// Abstract package manager interface
pub trait PackageManager: Send + Sync {
    /// Check if package is installed
    fn is_installed(&self, package: &str) -> Result<bool>;
    
    /// Get installed version
    fn get_version(&self, package: &str) -> Result<Option<String>>;
    
    /// Install package(s) - packages is Vec of (name, version) tuples
    fn install(&self, packages: &[(&str, &str)]) -> Result<()>;
    
    /// Remove package(s)
    fn remove(&self, packages: &[&str]) -> Result<()>;
    
    /// Update package(s)
    fn update(&self, packages: &[&str]) -> Result<()>;
    
    /// List all installed packages
    fn list_installed(&self) -> Result<Vec<InstalledPackage>>;
    
    /// Search for package
    fn search(&self, query: &str) -> Result<Vec<PackageInfo>>;
    
    /// Check for package conflicts
    fn check_conflicts(&self, packages: &[&str]) -> Result<Vec<String>>;
}

/// DNF-based package manager for Fedora
pub struct DNFPackageManager {
    use_sudo: bool,
}

impl DNFPackageManager {
    pub fn new(use_sudo: bool) -> Self {
        Self { use_sudo }
    }
    
    /// Execute a DNF command and return the output
    fn dnf_command(&self, args: &[&str]) -> Result<String> {
        // Check if dnf is available
        if !self.is_dnf_available() {
            return Err(DotfilesError::Path(
                "DNF not found. This feature requires DNF package manager (Fedora/RHEL).\n  💡 Install DNF or run on a Fedora-based system.".to_string()
            ));
        }
        
        let mut cmd = if self.use_sudo {
            let mut c = Command::new("sudo");
            c.arg("dnf");
            c
        } else {
            Command::new("dnf")
        };
        
        let output = cmd
            .args(args)
            .output()
            .map_err(|e| DotfilesError::Path(
                format!("Failed to execute DNF command: {}\n  💡 Make sure DNF is installed and in PATH", e)
            ))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DotfilesError::Path(
                format!("DNF command failed: {}\n  Command: dnf {}", stderr, args.join(" "))
            ));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    
    /// Check if DNF is available on the system
    fn is_dnf_available(&self) -> bool {
        Command::new("which")
            .arg("dnf")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

impl PackageManager for DNFPackageManager {
    fn is_installed(&self, package: &str) -> Result<bool> {
        match self.dnf_command(&["list", "installed", package]) {
            Ok(_) => Ok(true),
            Err(_) => Ok(false), // Package not installed returns error, we treat as false
        }
    }
    
    fn get_version(&self, package: &str) -> Result<Option<String>> {
        let output = match self.dnf_command(&["info", "installed", package]) {
            Ok(out) => out,
            Err(_) => return Ok(None), // Not installed
        };
        
        // Parse version from output
        // Format: "Version      : 2.41.0"
        for line in output.lines() {
            if line.starts_with("Version")
                && let Some(version) = line.split(':').nth(1) {
                    return Ok(Some(version.trim().to_string()));
                }
        }
        
        Ok(None)
    }
    
    fn install(&self, packages: &[(&str, &str)]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }
        
        let mut args = vec!["install", "-y"];
        let specs: Vec<String> = packages
            .iter()
            .map(|(name, version)| {
                if version == &"latest" {
                    name.to_string()
                } else {
                    format!("{}-{}", name, version)
                }
            })
            .collect();
        
        let spec_strs: Vec<&str> = specs.iter().map(|s| s.as_str()).collect();
        args.extend(spec_strs);
        
        self.dnf_command(&args)?;
        Ok(())
    }
    
    fn remove(&self, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }
        
        let mut args = vec!["remove", "-y"];
        args.extend(packages);
        self.dnf_command(&args)?;
        Ok(())
    }
    
    fn update(&self, packages: &[&str]) -> Result<()> {
        if packages.is_empty() {
            // Update all packages
            self.dnf_command(&["upgrade", "-y"])?;
        } else {
            let mut args = vec!["upgrade", "-y"];
            args.extend(packages);
            self.dnf_command(&args)?;
        }
        Ok(())
    }
    
    fn list_installed(&self) -> Result<Vec<InstalledPackage>> {
        let output = self.dnf_command(&["list", "installed", "--quiet"])?;
        
        let mut packages = Vec::new();
        
        // Skip header lines and parse package list
        let mut in_packages = false;
        for line in output.lines() {
            // Skip until we hit the "Installed Packages" section
            if line.contains("Installed Packages") {
                in_packages = true;
                continue;
            }
            
            if !in_packages {
                continue;
            }
            
            // Parse line format: "package.arch    version    repo"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                // Extract package name (remove .arch suffix)
                let name_with_arch = parts[0];
                let name = if let Some(dot_pos) = name_with_arch.rfind('.') {
                    &name_with_arch[..dot_pos]
                } else {
                    name_with_arch
                };
                
                packages.push(InstalledPackage {
                    name: name.to_string(),
                    version: parts[1].to_string(),
                    source: if parts.len() >= 3 {
                        parts[2].to_string()
                    } else {
                        "unknown".to_string()
                    },
                });
            }
        }
        
        Ok(packages)
    }
    
    fn search(&self, query: &str) -> Result<Vec<PackageInfo>> {
        let output = self.dnf_command(&["search", query])?;
        
        let mut packages = Vec::new();
        
        for line in output.lines() {
            if line.contains(".") && line.contains(":") {
                // This is a package name line: "package.arch : Description"
                if let Some(colon_pos) = line.find(':') {
                    let name_part = &line[..colon_pos].trim();
                    let desc_part = &line[colon_pos + 1..].trim();
                    
                    // Extract package name (remove .arch)
                    let name = if let Some(dot_pos) = name_part.rfind('.') {
                        &name_part[..dot_pos]
                    } else {
                        name_part
                    };
                    
                    packages.push(PackageInfo {
                        name: name.to_string(),
                        available_version: "unknown".to_string(), // DNF search doesn't show version
                        description: desc_part.to_string(),
                        source: PackageSource::Fedora,
                    });
                }
            }
        }
        
        Ok(packages)
    }
    
    fn check_conflicts(&self, packages: &[&str]) -> Result<Vec<String>> {
        // Use dnf repoquery to check for conflicts
        // This is a simplified implementation
        let mut conflicts = Vec::new();
        
        for package in packages {
            match self.dnf_command(&["repoquery", "--conflicts", package]) {
                Ok(output) => {
                    if !output.trim().is_empty() {
                        conflicts.push(format!("{}: {}", package, output.trim()));
                    }
                }
                Err(_) => {
                    // Ignore errors for conflict checking
                    continue;
                }
            }
        }
        
        Ok(conflicts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_dnf_manager_creation() {
        let manager = DNFPackageManager::new(false);
        assert!(!manager.use_sudo);
        
        let manager_sudo = DNFPackageManager::new(true);
        assert!(manager_sudo.use_sudo);
    }
}

