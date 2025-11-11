use crate::config::Config;
use crate::services::{ServiceManager, SystemdServiceManager};
use crate::utils::error::Result;
use colored::Colorize;

/// List all declared services from configuration
pub fn list_services(config: &Config, user_mode: bool) -> Result<()> {
    if config.services.is_empty() {
        println!("{}", "No services declared in configuration".yellow());
        println!("\n{}", "💡 Add services to your config.toml:".dimmed());
        println!("{}", r#"
[services.ssh]
package = "openssh-server"
enabled = true

[services.pipewire]
enabled = true
running = true
"#.dimmed());
        return Ok(());
    }
    
    let manager = SystemdServiceManager::new(user_mode);
    
    println!("\n{}", format!("Declared Services ({}):", config.services.len()).bold().cyan());
    println!("{}", format!("  Mode: {}", if user_mode { "user services" } else { "system services" }).dimmed());
    
    for (name, spec) in &config.services {
        let service_name = spec.name.as_ref().unwrap_or(name);
        
        // Try to get actual status
        let status_indicator = match manager.status(service_name) {
            Ok(status) => {
                let enabled_str = if status.enabled { "enabled" } else { "disabled" };
                let running_str = if status.running { "running" } else { "stopped" };
                format!("[{}, {}]", enabled_str, running_str)
            }
            Err(_) => "[unknown]".to_string(),
        };
        
        let expected = format!(
            "[{}, {}]",
            if spec.enabled { "enabled" } else { "disabled" },
            if spec.running.unwrap_or(false) { "running" } else { "stopped" }
        );
        
        println!(
            "  {} {} {} → {}",
            "•".dimmed(),
            service_name.green(),
            format!("expected: {}", expected).dimmed(),
            status_indicator.yellow()
        );
        
        if let Some(pkg) = &spec.package {
            println!("    {} package: {}", "↳".dimmed(), pkg.dimmed());
        }
    }
    
    Ok(())
}

/// Show status of a specific service
pub fn show_service_status(config: &Config, service: &str, user_mode: bool) -> Result<()> {
    let manager = SystemdServiceManager::new(user_mode);
    
    println!("{} Checking service status...\n", "→".cyan());
    
    match manager.status(service) {
        Ok(status) => {
            println!("{}", format!("Service: {}", status.name).bold().cyan());
            
            // Status indicators
            let enabled_indicator = if status.enabled {
                format!("  {} Enabled", "✓".green())
            } else {
                format!("  {} Disabled", "✗".red())
            };
            
            let running_indicator = if status.running {
                format!("  {} Running", "✓".green())
            } else {
                format!("  {} Stopped", "✗".red())
            };
            
            println!("{}", enabled_indicator);
            println!("{}", running_indicator);
            
            if !status.description.is_empty() {
                println!("  {} {}", "Description:".dimmed(), status.description);
            }
            
            // Check if declared in config
            if let Some(spec) = config.services.get(service) {
                println!("\n{}", "Configuration:".bold().dimmed());
                println!("  {} Declared in config", "✓".green());
                println!("  {} Should be enabled: {}", "→".dimmed(), spec.enabled);
                if let Some(should_run) = spec.running {
                    println!("  {} Should be running: {}", "→".dimmed(), should_run);
                }
            } else {
                println!("\n  {} Not declared in configuration", "⊘".yellow());
            }
        }
        Err(e) => {
            eprintln!("{} {}", "✗".red(), e);
            eprintln!("\n{}", "💡 This command requires systemctl (systemd)".yellow());
            eprintln!("{}", "   Make sure the service name is correct and systemd is installed.".yellow());
        }
    }
    
    Ok(())
}

/// Compare declared service states vs actual service states
pub fn compare_services(config: &Config, user_mode: bool) -> Result<()> {
    if config.services.is_empty() {
        println!("{}", "No services declared in configuration".yellow());
        return Ok(());
    }
    
    let manager = SystemdServiceManager::new(user_mode);
    
    println!("{} Comparing declared vs actual service states...\n", "→".cyan());
    println!("{}", "Service Status:".bold().cyan());
    
    let mut all_matched = true;
    
    for (name, spec) in &config.services {
        let service_name = spec.name.as_ref().unwrap_or(name);
        
        match manager.status(service_name) {
            Ok(status) => {
                let enabled_match = status.enabled == spec.enabled;
                let running_match = if let Some(should_run) = spec.running {
                    status.running == should_run
                } else {
                    true // Don't check if not specified
                };
                
                let matches = enabled_match && running_match;
                
                if matches {
                    println!(
                        "  {} {} [{}, {}]",
                        "✓".green(),
                        service_name,
                        if status.enabled { "enabled" } else { "disabled" },
                        if status.running { "running" } else { "stopped" }
                    );
                } else {
                    println!(
                        "  {} {} [{}, {}] {}",
                        "✗".red(),
                        service_name,
                        if status.enabled { "enabled" } else { "disabled" },
                        if status.running { "running" } else { "stopped" },
                        format!(
                            "(expected: {}, {})",
                            if spec.enabled { "enabled" } else { "disabled" },
                            if spec.running.unwrap_or(false) { "running" } else { "stopped" }
                        ).yellow()
                    );
                    all_matched = false;
                }
            }
            Err(_) => {
                println!(
                    "  {} {} {}",
                    "⚠".yellow(),
                    service_name,
                    "(cannot query status)".dimmed()
                );
                all_matched = false;
            }
        }
    }
    
    println!();
    
    if all_matched {
        println!("{} All declared services match their expected states", "✓".green());
    } else {
        println!("{} Some services don't match expected states", "⚠".yellow());
        println!("  💡 Service management will be available in future with 'flux apply'");
    }
    
    Ok(())
}

/// Enable a service
pub fn enable_service(service: &str, user_mode: bool) -> Result<()> {
    let manager = SystemdServiceManager::new(user_mode);
    
    println!("{} Enabling service '{}'...", "→".cyan(), service);
    
    match manager.enable(service) {
        Ok(_) => {
            println!("{} Service '{}' enabled", "✓".green(), service);
        }
        Err(e) => {
            eprintln!("{} Failed to enable service: {}", "✗".red(), e);
            return Err(e);
        }
    }
    
    Ok(())
}

/// Disable a service
pub fn disable_service(service: &str, user_mode: bool) -> Result<()> {
    let manager = SystemdServiceManager::new(user_mode);
    
    println!("{} Disabling service '{}'...", "→".cyan(), service);
    
    match manager.disable(service) {
        Ok(_) => {
            println!("{} Service '{}' disabled", "✓".green(), service);
        }
        Err(e) => {
            eprintln!("{} Failed to disable service: {}", "✗".red(), e);
            return Err(e);
        }
    }
    
    Ok(())
}

/// Start a service
pub fn start_service(service: &str, user_mode: bool) -> Result<()> {
    let manager = SystemdServiceManager::new(user_mode);
    
    println!("{} Starting service '{}'...", "→".cyan(), service);
    
    match manager.start(service) {
        Ok(_) => {
            println!("{} Service '{}' started", "✓".green(), service);
        }
        Err(e) => {
            eprintln!("{} Failed to start service: {}", "✗".red(), e);
            return Err(e);
        }
    }
    
    Ok(())
}

/// Stop a service
pub fn stop_service(service: &str, user_mode: bool) -> Result<()> {
    let manager = SystemdServiceManager::new(user_mode);
    
    println!("{} Stopping service '{}'...", "→".cyan(), service);
    
    match manager.stop(service) {
        Ok(_) => {
            println!("{} Service '{}' stopped", "✓".green(), service);
        }
        Err(e) => {
            eprintln!("{} Failed to stop service: {}", "✗".red(), e);
            return Err(e);
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_list_services_empty() {
        let config = Config::default();
        // Should not panic with empty services
        assert!(list_services(&config, true).is_ok());
    }
}

