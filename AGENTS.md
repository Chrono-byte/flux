# AGENTS.md - Flux Dotfiles Manager

Guidelines for AI agents working in this Rust codebase.

## Project Overview

Flux is a CLI dotfiles manager written in Rust 2024 edition. It uses symlink-based
synchronization between a repository (`~/.dotfiles`) and destination paths in the
user's home directory. The project includes a GUI component (`flux-gui/`) built
with a separate crate.

## Build & Development Commands

```bash
# Build
cargo build                    # Debug build
cargo build --release          # Release build

# Run
cargo run -- <subcommand>      # Run CLI (e.g., cargo run -- status)

# Lint & Format
cargo clippy                   # Run lints (strict - see Cargo.toml)
cargo fmt                      # Format code
cargo fmt -- --check           # Check formatting without changes

# Test
cargo test                     # Run all tests
cargo test <test_name>         # Run single test by name
cargo test --test unit_tests   # Run specific test file
cargo test -- --nocapture      # Show println! output in tests

# Documentation
cargo doc --open               # Generate and open docs
```

## Important Notes

Never perform a `git checkout`. This would lead only to a loss of all changes made to the repository.

## Code Style Guidelines

### Imports

Organize imports in this order, with blank lines between groups:

1. Standard library (`std::`)
2. External crates (alphabetical)
3. Internal crate modules (`crate::`)
4. Local module items (`use super::`, `use self::`)

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use colored::Colorize;
use serde::{Deserialize, Serialize};

use crate::types::TrackedFile;
use crate::utils::error::{DotfilesError, Result};
```

### Formatting (rustfmt.toml)

- **Max width**: 100 characters
- **Indentation**: 4 spaces (no tabs)
- **Newlines**: Unix style
- **Imports**: Automatically reordered

### Naming Conventions

- **Types/Structs/Enums**: `PascalCase` (e.g., `TrackedFile`, `SyncResult`)
- **Functions/Methods**: `snake_case` (e.g., `sync_files`, `get_repo_path`)
- **Constants**: `SCREAMING_SNAKE_CASE`
- **Modules**: `snake_case` (e.g., `file_manager`, `path_utils`)
- **Files**: Match module names, `snake_case.rs`

### Error Handling

This codebase uses `thiserror` for error definitions. Key rules:

1. **Never use `.unwrap()` or `.expect()`** - These are denied by clippy
2. **Never panic** - `panic!` is denied in production code
3. Use the custom `Result<T>` type alias from `utils::error`
4. Propagate errors with `?` operator
5. Create specific error variants with helpful context

```rust
use crate::utils::error::{DotfilesError, Result};

fn example() -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| DotfilesError::Config("Could not find home directory".to_string()))?;
    
    // Use ? for propagation
    let content = std::fs::read_to_string(&path)?;
    
    Ok(())
}
```

Error messages should include:
- What failed
- Why it failed
- A hint for resolution (prefixed with `\n  Solution: `)

### Documentation

Every public item requires documentation:

```rust
/// Brief description of the function.
///
/// # Parameters
///
/// * `config`: The configuration to use.
///
/// # Returns
///
/// Returns the result of the operation.
///
/// # Errors
///
/// Returns an error if the file cannot be read.
pub fn my_function(config: &Config) -> Result<()> { ... }
```

### Module Structure

Modules follow a consistent pattern:

```rust
// mod.rs or inline module

/// Module documentation.
pub mod submodule;

// Re-exports for convenience
pub use submodule::{PublicType, public_function};
```

### Testing Patterns

Tests use the standard Rust test framework:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_feature_behavior() {
        // Arrange
        let dir = tempdir().expect("test setup");
        
        // Act
        let result = function_under_test();
        
        // Assert
        assert!(result.is_ok());
    }
}
```

- Use `tempfile::tempdir()` for filesystem tests
- Test both success and error cases
- Use descriptive test names: `test_<what>_<condition>_<expected>`

## Architecture

### Module Layout

```
src/
  main.rs           # CLI entry point, command routing
  lib.rs            # Library exports
  types.rs          # Core type definitions
  file_manager.rs   # File/symlink operations
  
  commands/         # CLI subcommand implementations
    apply.rs        # Apply configuration
    status.rs       # Status checking
    ...
    
  config/           # Configuration loading/saving
    mod.rs          # Config struct and loading
    cli.rs          # CLI environment config
    profile.rs      # Profile management
    
  services/         # Business logic services
    git.rs          # Git operations
    hooks.rs        # Hook execution
    secrets.rs      # Secret management
    ...
    
  utils/            # Utility functions
    error.rs        # Error types
    dry_run.rs      # Dry-run tracking
    path_utils.rs   # Path manipulation
    ...
```

### Key Types

- `Config`: Main configuration struct, loaded from TOML
- `TrackedFile`: A file being managed (repo path, dest path, profile)
- `DotfilesError`: Custom error enum with variants for each error category
- `Result<T>`: Type alias for `std::result::Result<T, DotfilesError>`
- `DryRun`: Tracks operations for dry-run mode

### Common Patterns

**Dry-run support**: Most operations accept `is_dry_run: bool` and a `DryRun` tracker:

```rust
fn operation(dry_run_tracker: &mut DryRun, is_dry_run: bool) -> Result<()> {
    if is_dry_run {
        dry_run_tracker.log_operation(Operation::SomeOp { ... });
        return Ok(());
    }
    // Actual operation
}
```

**FileSystemManager**: Wraps filesystem operations with dry-run support:

```rust
let mut fs_manager = FileSystemManager::new(&mut dry_run_tracker, is_dry_run);
fs_manager.create_dir_all(&path)?;
fs_manager.copy(&src, &dst)?;
```

## Clippy Configuration

Strict linting is enabled in `Cargo.toml`. Key denied lints:

- `unwrap_used`: Use `ok_or_else()` or `?` instead
- `panic`: Handle errors gracefully
- `unsafe_code`: No unsafe blocks allowed
- `correctness`: All correctness lints are errors
- `suspicious`: All suspicious patterns are errors

Warned lints include:
- `pedantic`: Comprehensive style checks
- `complexity`: Keep code maintainable
- `missing_docs_in_private_items`: Document everything
- `too_many_arguments`: Refactor into structs

## Critical Rules

1. **Never run `git checkout`** - Avoid destroying state (see `.rules`)
2. **No panics in production** - All errors must be handled
3. **No unwrap/expect** - Use proper error handling
4. **No unsafe code** - Denied by lints
5. **Document all public items** - Required by lints
6. **Run `cargo clippy` before committing** - Must pass with no warnings

## Configuration

The config file is TOML format at `~/.config/flux/config.toml` or `~/.dotfiles/config.toml`.

Key sections:
- `[general]`: repo_path, current_profile, symlink_resolution
- `[tools.<name>]`: files = [{repo = "...", dest = "..."}]
- `[hooks]`: pre_apply, post_apply, pre_commit, post_commit
