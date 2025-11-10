use crate::config::Config;
use crate::error::{DotfilesError, Result};
use colored::Colorize;
use std::path::PathBuf;

pub fn create_profile(config: &mut Config, name: &str) -> Result<()> {
    let repo_path = config.get_repo_path()?;
    let profile_dir = repo_path.join("profiles").join(name);
    
    std::fs::create_dir_all(&profile_dir)?;
    
    println!("{} Created profile: {}", "✓".green(), name);
    Ok(())
}

pub fn switch_profile(config: &mut Config, name: &str) -> Result<()> {
    // Verify profile exists
    let repo_path = config.get_repo_path()?;
    let profile_dir = repo_path.join("profiles").join(name);
    
    if !profile_dir.exists() {
        return Err(DotfilesError::ProfileNotFound(name.to_string()));
    }
    
    config.general.current_profile = name.to_string();
    config.save()?;
    
    println!("{} Switched to profile: {}", "✓".green(), name);
    Ok(())
}

pub fn list_profiles(config: &Config) -> Result<Vec<String>> {
    let repo_path = config.get_repo_path()?;
    let profiles_dir = repo_path.join("profiles");
    
    if !profiles_dir.exists() {
        return Ok(vec!["default".to_string()]);
    }
    
    let mut profiles = vec!["default".to_string()];
    
    for entry in std::fs::read_dir(&profiles_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                profiles.push(name.to_string());
            }
        }
    }
    
    Ok(profiles)
}

pub fn get_profile_files(config: &Config, profile: &str) -> Result<Vec<(PathBuf, PathBuf)>> {
    let repo_path = config.get_repo_path()?;
    let profile_dir = repo_path.join("profiles").join(profile);
    
    if !profile_dir.exists() {
        return Ok(Vec::new());
    }
    
    let mut profile_files = Vec::new();
    
    // Walk through profile directory structure
    if profile_dir.is_dir() {
        for tool_entry in std::fs::read_dir(&profile_dir)? {
            let tool_entry = tool_entry?;
            let tool_path = tool_entry.path();
            
            if tool_path.is_dir() {
                for file_entry in std::fs::read_dir(&tool_path)? {
                    let file_entry = file_entry?;
                    let file_path = file_entry.path();
                    
                    if file_path.is_file() {
                        profile_files.push((file_path.clone(), file_path));
                    }
                }
            }
        }
    }
    
    Ok(profile_files)
}

