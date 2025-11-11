use crate::config::Config;
use crate::services::{DnfPackageManager, PackageManager};
use crate::utils::error::Result;
use colored::Colorize;

/// List all installed system packages
pub fn list_packages(_config: &Config, use_sudo: bool) -> Result<()> {
    let manager = DnfPackageManager::new(use_sudo);

    println!("{} Fetching installed packages...\n", "→".cyan());

    match manager.list_installed() {
        Ok(packages) => {
            if packages.is_empty() {
                println!("{} No packages found", "⊘".yellow());
                return Ok(());
            }

            println!(
                "{}",
                format!("Installed Packages ({}):", packages.len())
                    .bold()
                    .cyan()
            );
            for pkg in packages.iter().take(50) {
                // Limit to first 50 for readability
                println!(
                    "  {} {} {}",
                    "•".dimmed(),
                    pkg.name.green(),
                    format!("({})", pkg.version).dimmed()
                );
            }

            if packages.len() > 50 {
                println!(
                    "\n  {} ... and {} more packages",
                    "•".dimmed(),
                    packages.len() - 50
                );
                println!("  💡 Use 'dnf list installed' to see all packages");
            }
        }
        Err(e) => {
            eprintln!("{} {}", "✗".red(), e);
            eprintln!(
                "\n{}",
                "💡 This command requires DNF (Fedora/RHEL package manager)".yellow()
            );
            eprintln!(
                "{}",
                "   If you're not on Fedora, package management is not available.".yellow()
            );
        }
    }

    Ok(())
}

/// Show packages declared in configuration
pub fn show_declared_packages(config: &Config) -> Result<()> {
    if config.packages.is_empty() {
        println!("{}", "No packages declared in configuration".yellow());
        println!("\n{}", "💡 Add packages to your config.toml:".dimmed());
        println!(
            "{}",
            r#"
[packages.git]
version = "latest"
description = "Version control"

[packages.neovim]
version = "latest"
"#
            .dimmed()
        );
        return Ok(());
    }

    println!(
        "\n{}",
        format!("Declared Packages ({}):", config.packages.len())
            .bold()
            .cyan()
    );

    for (name, spec) in &config.packages {
        let version_str = format!("({})", spec.version).yellow();
        println!("  {} {} {}", "•".dimmed(), name.green(), version_str);

        if let Some(desc) = &spec.description {
            println!("    {}", desc.dimmed());
        }
    }

    Ok(())
}

/// Compare declared packages vs installed packages
pub fn compare_packages(config: &Config, use_sudo: bool) -> Result<()> {
    if config.packages.is_empty() {
        println!("{}", "No packages declared in configuration".yellow());
        return Ok(());
    }

    let manager = DnfPackageManager::new(use_sudo);

    println!(
        "{} Comparing declared vs installed packages...\n",
        "→".cyan()
    );

    let installed = match manager.list_installed() {
        Ok(pkgs) => pkgs,
        Err(e) => {
            eprintln!("{} {}", "✗".red(), e);
            eprintln!(
                "\n{}",
                "💡 Cannot compare packages without DNF access".yellow()
            );
            return Ok(());
        }
    };

    let mut installed_map = std::collections::HashMap::new();
    for pkg in installed {
        installed_map.insert(pkg.name.clone(), pkg.version);
    }

    println!("{}", "Package Status:".bold().cyan());

    let mut all_matched = true;

    // Check declared packages
    for (name, spec) in &config.packages {
        if let Some(version) = installed_map.get(name) {
            let matches = spec.version == "latest" || spec.version == *version;

            if matches {
                println!(
                    "  {} {} {} {}",
                    "✓".green(),
                    name,
                    "→".dimmed(),
                    version.dimmed()
                );
            } else {
                println!(
                    "  {} {} {} {} {}",
                    "⚠".yellow(),
                    name,
                    "→".dimmed(),
                    version,
                    format!("(expected {})", spec.version).yellow()
                );
                all_matched = false;
            }
        } else {
            println!("  {} {} {}", "✗".red(), name, "(not installed)".red());
            all_matched = false;
        }
    }

    // Show extra installed packages (not in config)
    let extra_packages: Vec<_> = installed_map
        .keys()
        .filter(|name| !config.packages.contains_key(*name))
        .collect();

    if !extra_packages.is_empty() {
        println!(
            "\n{}",
            "Packages installed but not in config:".bold().dimmed()
        );
        for name in extra_packages.iter().take(10) {
            if let Some(version) = installed_map.get(*name) {
                println!(
                    "  {} {} {}",
                    "⊕".yellow(),
                    name,
                    format!("({})", version).dimmed()
                );
            }
        }
        if extra_packages.len() > 10 {
            println!(
                "  {} ... and {} more",
                "⊕".yellow(),
                extra_packages.len() - 10
            );
        }
    }

    println!();

    if all_matched {
        println!(
            "{} All declared packages are installed and match",
            "✓".green()
        );
    } else {
        println!(
            "{} Some packages are missing or have version mismatches",
            "⚠".yellow()
        );
        println!("  💡 Package installation will be available in future with 'flux apply'");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PackageSpec;

    #[test]
    fn test_show_declared_packages_empty() {
        let config = Config::default();
        // Should not panic with empty packages
        assert!(show_declared_packages(&config).is_ok());
    }

    #[test]
    fn test_show_declared_packages_with_data() {
        let mut config = Config::default();
        config
            .packages
            .insert("git".to_string(), PackageSpec::new("git".to_string()));

        assert!(show_declared_packages(&config).is_ok());
    }
}
