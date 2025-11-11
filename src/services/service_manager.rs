use crate::utils::error::{DotfilesError, Result};
use std::process::Command;

/// Abstract service manager interface
pub trait ServiceManager: Send + Sync {
    /// Check if service is enabled
    fn is_enabled(&self, service: &str) -> Result<bool>;
    
    /// Check if service is running
    fn is_running(&self, service: &str) -> Result<bool>;
    
    /// Enable service (start on boot/login)
    fn enable(&self, service: &str) -> Result<()>;
    
    /// Disable service
    fn disable(&self, service: &str) -> Result<()>;
    
    /// Start service now
    fn start(&self, service: &str) -> Result<()>;
    
    /// Stop service now
    fn stop(&self, service: &str) -> Result<()>;
    
    /// Restart service
    fn restart(&self, service: &str) -> Result<()>;
    
    /// Get service status information
    fn status(&self, service: &str) -> Result<ServiceStatus>;
}

#[derive(Debug, Clone)]
pub struct ServiceStatus {
    pub name: String,
    pub enabled: bool,
    pub running: bool,
    pub description: String,
}

/// Systemd user/system service manager
pub struct SystemdServiceManager {
    /// true = --user (user services), false = system services
    user_mode: bool,
}

impl SystemdServiceManager {
    pub fn new(user_mode: bool) -> Self {
        Self { user_mode }
    }
    
    /// Get systemctl arguments based on mode
    fn systemctl_args(&self) -> Vec<&'static str> {
        if self.user_mode {
            vec!["--user"]
        } else {
            vec![]
        }
    }
    
    /// Run a systemctl command
    fn run_systemctl(&self, args: &[&str]) -> Result<()> {
        // Check if systemctl is available
        if !self.is_systemctl_available() {
            return Err(DotfilesError::Path(
                "systemctl not found. This feature requires systemd.\n  💡 Make sure systemd is installed and systemctl is in PATH.".to_string()
            ));
        }
        
        let mut cmd = if !self.user_mode {
            // System services may need sudo
            let mut c = Command::new("sudo");
            c.arg("systemctl");
            c
        } else {
            Command::new("systemctl")
        };
        
        cmd.args(self.systemctl_args());
        cmd.args(args);
        
        let output = cmd
            .output()
            .map_err(|e| DotfilesError::Path(
                format!("Failed to execute systemctl: {}\n  💡 Make sure systemctl is installed", e)
            ))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DotfilesError::Path(
                format!("systemctl command failed: {}\n  Command: systemctl {} {}", 
                    stderr, 
                    if self.user_mode { "--user" } else { "" },
                    args.join(" "))
            ));
        }
        
        Ok(())
    }
    
    /// Run systemctl command and return output
    fn run_systemctl_output(&self, args: &[&str]) -> Result<String> {
        if !self.is_systemctl_available() {
            return Err(DotfilesError::Path(
                "systemctl not found. This feature requires systemd.".to_string()
            ));
        }
        
        let mut cmd = if !self.user_mode {
            let mut c = Command::new("sudo");
            c.arg("systemctl");
            c
        } else {
            Command::new("systemctl")
        };
        
        cmd.args(self.systemctl_args());
        cmd.args(args);
        
        let output = cmd
            .output()
            .map_err(|e| DotfilesError::Path(
                format!("Failed to execute systemctl: {}", e)
            ))?;
        
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
    
    /// Check if systemctl is available
    fn is_systemctl_available(&self) -> bool {
        Command::new("which")
            .arg("systemctl")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

impl ServiceManager for SystemdServiceManager {
    fn is_enabled(&self, service: &str) -> Result<bool> {
        let mut cmd = if !self.user_mode {
            let mut c = Command::new("sudo");
            c.arg("systemctl");
            c
        } else {
            Command::new("systemctl")
        };
        
        cmd.args(self.systemctl_args());
        cmd.args(["is-enabled", service]);
        
        let output = cmd
            .output()
            .map_err(|e| DotfilesError::Path(
                format!("Failed to check if service is enabled: {}", e)
            ))?;
        
        Ok(output.status.success())
    }
    
    fn is_running(&self, service: &str) -> Result<bool> {
        let mut cmd = if !self.user_mode {
            let mut c = Command::new("sudo");
            c.arg("systemctl");
            c
        } else {
            Command::new("systemctl")
        };
        
        cmd.args(self.systemctl_args());
        cmd.args(["is-active", service]);
        
        let output = cmd
            .output()
            .map_err(|e| DotfilesError::Path(
                format!("Failed to check if service is running: {}", e)
            ))?;
        
        Ok(output.status.success())
    }
    
    fn enable(&self, service: &str) -> Result<()> {
        self.run_systemctl(&["enable", service])
    }
    
    fn disable(&self, service: &str) -> Result<()> {
        self.run_systemctl(&["disable", service])
    }
    
    fn start(&self, service: &str) -> Result<()> {
        self.run_systemctl(&["start", service])
    }
    
    fn stop(&self, service: &str) -> Result<()> {
        self.run_systemctl(&["stop", service])
    }
    
    fn restart(&self, service: &str) -> Result<()> {
        self.run_systemctl(&["restart", service])
    }
    
    fn status(&self, service: &str) -> Result<ServiceStatus> {
        let enabled = self.is_enabled(service).unwrap_or(false);
        let running = self.is_running(service).unwrap_or(false);
        
        // Get description from status output
        let status_output = self.run_systemctl_output(&["status", service])
            .unwrap_or_default();
        
        let mut description = String::new();
        for line in status_output.lines() {
            if line.trim().starts_with("Description:") {
                description = line.trim()
                    .strip_prefix("Description:")
                    .unwrap_or("")
                    .trim()
                    .to_string();
                break;
            }
        }
        
        Ok(ServiceStatus {
            name: service.to_string(),
            enabled,
            running,
            description,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_systemd_manager_creation() {
        let manager = SystemdServiceManager::new(true);
        assert!(manager.user_mode);
        
        let manager_system = SystemdServiceManager::new(false);
        assert!(!manager_system.user_mode);
    }
    
    #[test]
    fn test_systemctl_args() {
        let user_manager = SystemdServiceManager::new(true);
        assert_eq!(user_manager.systemctl_args(), vec!["--user"]);
        
        let system_manager = SystemdServiceManager::new(false);
        assert_eq!(system_manager.systemctl_args(), Vec::<&str>::new());
    }
}


