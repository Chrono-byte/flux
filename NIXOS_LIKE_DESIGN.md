# Making Flux More Like NixOS

## Overview

NixOS is a declarative Linux distribution where system state is entirely described through configuration files. Making flux more NixOS-like means:

1. **Pure Declarativity**: Configuration files are the single source of truth
2. **Reproducibility**: Same configuration always produces identical state
3. **Atomic Operations**: Systems reach desired state or fail atomically
4. **Immutability**: Configuration changes are tracked and reversible
5. **Composability**: Modular, reusable configuration building blocks
6. **Generations/Rollback**: Easy rollback to previous configurations

## Current State vs. NixOS-like

### Current Flux Architecture
- **Imperative commands**: `flux add`, `flux sync` actively modify state
- **Mutable config**: Files can be edited ad-hoc, state can diverge
- **Command-driven**: Behavior depends on sequence of commands
- **State scattered**: Configuration in TOML, actual state in filesystem and git

### NixOS-like Flux
- **Declarative configuration**: Single canonical config file describes entire state
- **Idempotent operations**: `flux apply` always reaches desired state
- **Deterministic**: Running the same config produces identical results
- **Version control**: Full history with named generations
- **Verification**: `flux status` shows actual vs. desired state
- **Composability**: Include and extend configurations

## Proposed Architecture Changes

### 1. Declarative Configuration Model

**Current**:
```toml
[tools.sway]
files = [
    { repo = "config", dest = ".config/sway/config" }
]
```

**NixOS-like**:
```toml
[configuration]
enable = true
description = "Home system configuration"
version = "1.0"

[configuration.files."sway"]
description = "Sway window manager configuration"
source = "config"
target = ".config/sway/config"
symlink = true
backup = true

[configuration.files."firefox"]
source = "browser/firefox"
target = ".mozilla/firefox"
symlink = true
profile = "home"

[configuration.packages]
# Optional: ensure programs are available
sway = { ensure_installed = true }
firefox = { ensure_installed = true }
```

### 2. State Specification and Verification

**Add to config**:
```toml
[configuration.validation]
# Verify desired state without modifying
check_symlinks = true
verify_contents = true
detect_drift = true

[configuration.generation]
# Named versions for rollback
enabled = true
max_generations = 50
keep_days = 30
```

### 3. Composable Configuration

Instead of just profiles, support composition:

```toml
# home.toml - Base home configuration
[configuration]
include = ["profiles/base.toml", "profiles/development.toml"]

# profiles/base.toml
[configuration.files."nvim"]
source = "config/nvim"
target = ".config/nvim"

# profiles/development.toml - Extends base
[configuration.files."rust-tools"]
source = "config/rust"
target = ".config/rust"
```

### 4. New Core Operations

#### `flux apply` (NixOS-like)
```bash
# Idempotent: reaches desired state
flux apply
flux apply --generation=5  # Rollback to generation 5
flux apply --profile work  # Apply specific profile
flux apply --dry-run       # Preview changes
```

**Behavior**:
1. Load declarative configuration
2. Check current state vs. desired
3. Show diff/preview
4. Apply changes atomically
5. Create generation checkpoint
6. Verify final state

#### `flux status` (Enhanced)
```bash
# Show desired vs. actual state
flux status --json     # Machine-readable format
flux status --verbose  # Show detailed discrepancies
```

**Output**:
```
Configuration Status: home (generation 42)
Created: 2024-11-11 10:30:00

Files:
  ✓ sway/config → ~/.config/sway/config (symlink, matched)
  ✗ waybar/config → ~/.config/waybar/config (missing symlink)
  ↻ firefox prefs → ~/.mozilla/... (modified since last apply)

Generation History:
  42: 2024-11-11 10:30:00 - Applied home profile
  41: 2024-11-11 09:15:00 - Added firefox configuration
  40: 2024-11-10 15:45:00 - Updated sway config
```

#### `flux generations`
```bash
flux generations list
flux generations diff 42 41   # Compare states
flux generations rollback 40  # Rollback to generation 40
flux generations delete 1-39  # Clean up old generations
```

#### `flux diff`
```bash
flux diff                    # Show changes needed
flux diff --generation=5     # Diff against generation 5
flux diff --profile work     # Diff between profiles
```

### 5. Enhanced Profile System

```toml
[profiles.base]
description = "Base configuration"
enabled = true
priority = 1

[profiles.base.files]
nvim = { source = "config/nvim", target = ".config/nvim" }
sway = { source = "config/sway", target = ".config/sway" }

[profiles.work]
description = "Work laptop configuration"
inherits = ["base"]
priority = 2

[profiles.work.files]
sway = { source = "config/sway-work", target = ".config/sway" }  # Override
aws = { source = "config/aws", target = ".aws" }  # Add

[profiles.laptop]
inherits = ["base"]
priority = 2
hostname_match = "laptop"
enable_if = 'env.DEVICE == "laptop"'
```

### 6. Atomic Transactions

```rust
// New transaction model
impl Config {
    /// Atomically apply configuration changes
    pub fn apply(&self, dry_run: bool) -> Result<Generation> {
        let tx = Transaction::begin(&self)?;
        
        // Phase 1: Validate
        tx.validate_all_files()?;
        
        // Phase 2: Prepare (create symlinks in temp space)
        tx.prepare_changes()?;
        
        // Phase 3: Commit (atomic move into place)
        tx.commit()?;
        
        // Phase 4: Verify
        tx.verify_state()?;
        
        // Phase 5: Record generation
        let gen = tx.finalize()?;
        
        Ok(gen)
    }
}
```

### 7. Declarative vs. Imperative Bridge

Keep backward compatibility while providing NixOS-like workflow:

```bash
# Imperative (existing, still works)
flux file add sway ~/.config/sway/config
flux sync

# Declarative (new NixOS-like way)
# (Edit config.toml)
flux apply

# View as code
flux export              # Show config that would produce current state
flux diff               # Show changes needed to reach desired state
```

### 8. Generation/Version Management

```rust
#[derive(Debug, Serialize)]
pub struct Generation {
    pub id: u32,
    pub timestamp: DateTime<Utc>,
    pub profile: String,
    pub description: String,
    pub config_hash: String,  // Determinism check
    pub state_hash: String,   // What was actually applied
    pub changes: Vec<FileChange>,
    pub previous_gen: Option<u32>,
}

// Stored in ~/.dotfiles/.generations/
// 01-2024-11-11-10-30-00-base.json
// 02-2024-11-11-09-15-00-home.json
```

## Implementation Roadmap

### Phase 1: Foundation (No Breaking Changes)
- [ ] Add `Generation` struct and storage
- [ ] Implement `flux generations list`
- [ ] Add generation recording to existing `flux sync`
- [ ] Implement `flux status --generation=N`

### Phase 2: Atomic Transactions
- [ ] Implement `Transaction` system
- [ ] Make existing `flux sync` use transactions
- [ ] Add rollback support
- [ ] Enhance validation system

### Phase 3: Declarative Workflow
- [ ] Add `flux apply` command (wraps transactional sync)
- [ ] Implement composition/includes in config
- [ ] Add `flux diff` command
- [ ] Enhanced profile inheritance

### Phase 4: Verification & Drift Detection
- [ ] Implement `flux check` (verify vs. desired state)
- [ ] Add continuous drift detection
- [ ] Implement `flux status` enhancements
- [ ] Add verification reports

### Phase 5: Advanced Features
- [ ] Conditional includes based on hostname/environment
- [ ] Package existence checking
- [ ] Policy enforcement
- [ ] Custom hooks/scripts

## Benefits of NixOS-like Approach

1. **Predictability**: Exact same config always produces same result
2. **Safety**: Can always roll back, changes are non-destructive
3. **Clarity**: Config file is source of truth, not imperative commands
4. **Auditability**: Full history of all changes with generations
5. **Reproducibility**: Share exact config, get exact results
6. **Composability**: Build complex configs from simple pieces
7. **Automation**: Declarative format easier to generate/transform

## Example Workflow Transformation

### Current Workflow
```bash
# Add files
flux add nvim ~/.config/nvim/init.vim
flux add sway ~/.config/sway/config
flux add firefox ~/.mozilla/firefox/profile

# Sync
flux sync

# Make change to browser profile
# (file modified)

# Re-sync to pick up changes
flux sync

# Later, need to revert
# (manual process or restore from backup)
```

### NixOS-like Workflow
```bash
# Edit configuration
cat > ~/.config/flux/config.toml << 'EOF'
[configuration.files.nvim]
source = "config/nvim/init.vim"
target = ".config/nvim/init.vim"

[configuration.files.sway]
source = "config/sway/config"
target = ".config/sway/config"

[configuration.files.firefox]
source = "config/firefox"
target = ".mozilla/firefox"
EOF

# Apply configuration (idempotent)
flux apply

# Check status (shows desired vs. actual)
flux status

# Make changes in repo, reapply
flux apply

# Show what changed
flux diff

# Rollback to previous state
flux generations list
flux generations rollback 5

# Or apply old generation directly
flux apply --generation=5
```

## Configuration File Evolution

```toml
# Current (remains compatible)
[general]
repo_path = "~/.dotfiles"

[tools.sway]
files = [{ repo = "config", dest = ".config/sway/config" }]

# Enhanced (new NixOS-like sections, backwards compatible)
[general]
repo_path = "~/.dotfiles"
enable_generations = true
enable_atomic_apply = true

[tools.sway]
files = [{ repo = "config", dest = ".config/sway/config" }]

# New declarative section (can be primary)
[configuration]
description = "Home system"

[configuration.files.sway]
source = "config"
target = ".config/sway/config"
```

## Compatibility Strategy

1. **Keep existing commands working** (add deprecation warnings eventually)
2. **New commands don't break old workflow**
3. **New `flux apply` doesn't require changing existing configs**
4. **Gradual migration path** for users
5. **Documentation showing both approaches**

## Migration Path for Users

```bash
# Step 1: Export current state
flux export > current-state.toml

# Step 2: Start using new workflow
# (edit ~/.config/flux/config.toml to add [configuration] section)

# Step 3: Use new apply
flux apply --dry-run  # Preview

flux apply            # Apply with generations

# Step 4: Stop using imperative commands
# flux sync → flux apply
# flux restore → flux generations rollback
```

## Key Design Principles

1. **Declarative > Imperative**: Configuration describes desired state
2. **Immutable History**: Generations are immutable records
3. **Idempotent**: Same config applied multiple times = same result
4. **Composable**: Build complex configs from simple pieces
5. **Verifiable**: Can check if actual state matches desired state
6. **Reversible**: Always possible to go back to previous state
7. **Transparent**: All changes visible in history and generation records

