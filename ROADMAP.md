# Flux: Unified Development Roadmap

**Vision**: Transform Flux into a declarative system layer for Fedora Linux, providing NixOS-like reproducibility and safety while working with existing Fedora tools.

**Status**: Phase 1 Complete ✅ (as of 2024)

---

## Quick Overview

| Phase | Focus | Duration | Status | Priority |
|-------|-------|----------|--------|----------|
| **Phase 1** | Foundation - Config & Abstractions | 2 weeks | ✅ Complete | HIGH |
| **Phase 2** | Generation System | 2 weeks | 🔲 Pending | HIGH |
| **Phase 3** | Atomic Transactions | 2 weeks | 🔲 Pending | HIGH |
| **Phase 4** | Declarative Apply Command | 2 weeks | 🔲 Pending | HIGH |
| **Phase 5** | State Verification & Drift Detection | 2 weeks | 🔲 Pending | MEDIUM |
| **Phase 6** | Configuration Composition | 2 weeks | 🔲 Pending | MEDIUM |
| **Phase 7** | Package Operations Integration | 2 weeks | 🔲 Pending | MEDIUM |
| **Phase 8** | Testing & Documentation | 2 weeks | 🔲 Pending | HIGH |

**Total Timeline**: 16 weeks (4 months) for complete implementation

---

## Phase 1: Foundation ✅ COMPLETE

**Goal**: Extend configuration format and create package/service management abstractions

**Duration**: 2 weeks | **Status**: ✅ Complete

### Completed Features

#### Configuration Extension

- ✅ Extended `Config` struct with `packages`, `services`, `environment` fields
- ✅ Added `PackageSpec`, `ServiceSpec`, `EnvironmentSpec` types
- ✅ 100% backward compatibility maintained
- ✅ All new fields use `#[serde(default)]`

#### Package Management Abstraction

- ✅ Created `PackageManager` trait with 8 core methods
- ✅ Implemented `DNFPackageManager` for Fedora
- ✅ Non-sudo by default, `--sudo` flag available
- ✅ Graceful error handling when DNF unavailable

#### Service Management Abstraction

- ✅ Created `ServiceManager` trait with 7 core methods
- ✅ Implemented `SystemdServiceManager` with user/system mode
- ✅ User services by default, `--system` flag available
- ✅ Graceful error handling when systemctl unavailable

#### New CLI Commands

- ✅ `flux package show` - Display declared packages
- ✅ `flux package list [--sudo]` - List installed packages
- ✅ `flux package status [--sudo]` - Compare declared vs installed
- ✅ `flux service list [--system]` - Show declared services
- ✅ `flux service status <name> [--system]` - Service details
- ✅ `flux service compare [--system]` - Compare states
- ✅ `flux service enable/disable/start/stop <name> [--system]`

### Configuration Example

```toml
[packages.git]
version = "latest"
description = "Version control"

[services.ssh]
enabled = true
package = "openssh-server"

[environment]
EDITOR = "nvim"
shell = "bash"
```

### Metrics

- **Lines Added**: ~1,262
- **Files Created**: 4 new files
- **Files Modified**: 5 existing files
- **Test Coverage**: All core features tested
- **Build Status**: ✅ All green

---

## Phase 2: Generation System

**Goal**: Implement version history and rollback capability

**Duration**: 2 weeks | **Status**: 🔲 Pending | **Priority**: HIGH

### Objectives

#### Generation Types (`src/types.rs`)

- [ ] Add `Generation` struct with:
  - `id: u32` - Unique generation number
  - `created: DateTime<Utc>` - Timestamp
  - `profile: String` - Profile name
  - `description: String` - User description
  - `config_hash: String` - Configuration checksum
  - `state_hash: String` - Resulting state checksum
  - `changes: Vec<FileChange>` - What changed
  - `previous_id: Option<u32>` - Parent generation
  - `metadata: HashMap<String, String>` - Extensible metadata

- [ ] Add `GenerationManifest` struct
- [ ] Add `FileChange` enum variants

#### Generation Manager Service (`src/services/generations.rs` - NEW)

- [ ] Implement `GenerationManager` for CRUD operations
- [ ] Store generations in `.generations/` directory as JSON
- [ ] Methods:
  - `save_generation()` - Persist new generation
  - `load_generations()` - Load all generations
  - `get_generation(id)` - Get specific generation
  - `list_generations()` - List with filtering
  - `cleanup(keep, days)` - Remove old generations
  - `current_id()` - Get active generation
  - `next_id()` - Get next available ID

#### Generation Commands (`src/commands/generations.rs` - NEW)

- [ ] `flux generations list` - Show all generations
  - Display: ID, timestamp, profile, description
  - Indicate current generation with marker
  - Show number of changes per generation
  
- [ ] `flux generations show <id>` - Display generation details
  - Full metadata
  - List of all changes
  - Config and state hashes
  
- [ ] `flux generations rollback <id>` - Revert to generation
  - Restore file states from generation record
  - Create new generation for rollback
  - Atomic operation
  
- [ ] `flux generations cleanup` - Remove old generations
  - Keep N most recent (default: 20)
  - Keep all from last N days (default: 30)
  - Dry-run mode

#### CLI Integration

- [ ] Add `Generations` command variant to main.rs
- [ ] Add `GenerationCommands` enum
- [ ] Implement `handle_generation_command()`
- [ ] Update help text

### Success Criteria

- [ ] Every file operation creates a generation
- [ ] Generations are stored as JSON in `.generations/`
- [ ] `flux generations list` shows all history
- [ ] `flux generations rollback` restores previous states
- [ ] Generation cleanup works with retention policies

### Testing

- [ ] Test generation creation
- [ ] Test generation listing
- [ ] Test generation rollback
- [ ] Test cleanup with various retention policies
- [ ] Test generation ID sequencing

---

## Phase 3: Atomic Transaction System

**Goal**: All operations succeed or fail atomically, never partial state

**Duration**: 2 weeks | **Status**: 🔲 Pending | **Priority**: HIGH

### Objectives

#### Transaction Types (`src/services/transactions.rs` - NEW)

- [ ] Create `Transaction` struct with:
  - `id: String` - UUID
  - `state: TransactionState` - Current phase
  - `temp_dir: PathBuf` - Staging area
  - `operations: Vec<FileOperation>` - Planned operations
  - `results: Vec<OperationResult>` - Execution results

- [ ] Create `TransactionState` enum:
  - `Started` - Transaction begun
  - `Prepared` - Validation passed
  - `Committed` - Changes applied
  - `Verified` - State confirmed
  - `RolledBack` - Reverted

- [ ] Create `FileOperation` enum:
  - `CreateSymlink { source, target }`
  - `RemoveSymlink { target }`
  - `BackupAndReplace { source, target, backup_dir }`
  - `InstallPackage { name, version }`
  - `RemovePackage { name }`
  - `EnableService { name }`
  - `DisableService { name }`

#### Transaction Lifecycle

- [ ] Implement transaction phases:
  1. **Validate** - Check all operations will succeed
  2. **Prepare** - Stage changes in temp directory
  3. **Commit** - Atomically apply all changes
  4. **Verify** - Confirm all changes applied correctly
  5. **Record** - Create generation snapshot
  6. **Rollback** - If any phase fails, undo all changes

#### Transaction Methods

- [ ] `Transaction::begin(id, temp_dir)` - Start new transaction
- [ ] `add_operation(op)` - Queue operation
- [ ] `prepare()` - Validate all operations
- [ ] `commit()` - Execute all operations
- [ ] `verify()` - Confirm success
- [ ] `rollback()` - Undo all changes
- [ ] `get_changes()` - Get list of changes

#### Integration with File Manager

- [ ] Update `src/file_manager.rs` to use transactions
- [ ] Replace direct file operations with transaction operations
- [ ] Add transaction wrapper for sync operations

### Success Criteria

- [ ] All file operations go through transactions
- [ ] Failed operations trigger automatic rollback
- [ ] No partial states possible
- [ ] Transaction state is always consistent
- [ ] Temp directory cleaned up after completion

### Testing

- [ ] Test successful transaction commit
- [ ] Test transaction rollback on failure
- [ ] Test partial failure scenarios
- [ ] Test verification phase
- [ ] Test cleanup of temp directories

---

## Phase 4: Declarative Apply Command

**Goal**: Single idempotent command to synchronize system to declared state

**Duration**: 2 weeks | **Status**: 🔲 Pending | **Priority**: HIGH

### Objectives

#### Core Apply Implementation (`src/commands/apply.rs` - NEW)

- [ ] Read configuration file as source of truth
- [ ] Compare declared state vs actual system state
- [ ] Generate list of operations needed:
  - Packages to install/remove
  - Configs to sync
  - Services to enable/start/disable/stop
- [ ] Execute via transaction system
- [ ] Create generation record after success

#### Apply Features

- [ ] `flux apply` - Apply current configuration
  - Read `~/.config/flux/config.toml`
  - Compute diff between declared and actual state
  - Show preview of changes
  - Prompt for confirmation (unless `--yes`)
  - Execute through transaction system
  - Create new generation

- [ ] `flux apply --dry-run` - Preview what would change
  - Show all operations that would be performed
  - No actual changes
  - Safe to run anytime

- [ ] `flux apply --profile <name>` - Apply specific profile
  - Load profile-specific configuration
  - Apply only that profile's changes

- [ ] `flux apply --generation <id>` - Rollback to generation
  - Load generation state
  - Revert to that exact state
  - Create new generation for rollback

- [ ] `flux apply --description <text>` - Add generation description
  - Annotate generation with custom message
  - Useful for marking important states

- [ ] `flux apply --yes` - Skip confirmation prompts
  - Auto-confirm all operations
  - Useful for automation/scripts

#### State Comparison Engine

- [ ] Compare packages: declared vs installed
- [ ] Compare services: expected states vs actual
- [ ] Compare files: repo vs deployed
- [ ] Generate minimal set of operations
- [ ] Optimize operation order (packages → files → services)

#### Integration Points

- [ ] Integrate package installation (`PackageManager::install()`)
- [ ] Integrate file syncing (existing `sync_files()`)
- [ ] Integrate service management (`ServiceManager::enable/start()`)
- [ ] Wrap all in transaction system
- [ ] Generate generation record

### Success Criteria

- [ ] `flux apply` is idempotent (run twice = same result)
- [ ] All changes are atomic (all or nothing)
- [ ] Clear preview of changes before applying
- [ ] Confirmation prompts prevent accidents
- [ ] Generation created for every apply
- [ ] Dry-run mode shows accurate preview

### Testing

- [ ] Test idempotence (apply twice)
- [ ] Test dry-run accuracy
- [ ] Test package installation
- [ ] Test service management
- [ ] Test file syncing
- [ ] Test rollback via apply
- [ ] Test profile switching

---

## Phase 5: State Verification & Drift Detection

**Goal**: Show what differs between declared and actual state

**Duration**: 2 weeks | **Status**: 🔲 Pending | **Priority**: MEDIUM

### Objectives

#### State Comparison (`src/services/verification.rs` - NEW)

- [ ] Create `SystemState` struct:
  - `declared: DeclaredState` - From config
  - `actual: ActualState` - From system
  - `drifts: Vec<StateDrift>` - Differences

- [ ] Create `StateDrift` enum:
  - `PackageMissing(String)` - Package not installed
  - `PackageVersionWrong { package, expected, actual }`
  - `ConfigFileDrift { path, expected, actual }`
  - `ServiceNotEnabled(String)`
  - `ServiceNotRunning(String)`
  - `EnvironmentVariableWrong { name, expected, actual }`

- [ ] Implement `verify_system_state(config)`:
  - Extract declared state from config
  - Query actual system state
  - Compare and detect drifts
  - Return comprehensive report

#### Enhanced Status Command (`src/commands/status.rs`)

- [ ] Show current generation information
- [ ] Display all tracked items with indicators:
  - ✓ Matched (green)
  - ✗ Mismatch (red)
  - ⊕ Extra (yellow, not in config)
  - ⊘ Missing (yellow, in config but not present)
- [ ] Highlight drifts with details
- [ ] Suggest `flux apply` to fix drifts
- [ ] Show summary statistics

#### Diff Command (`src/commands/diff.rs` - NEW)

- [ ] `flux diff` - Show what `flux apply` would change
  - Compare current state vs declared
  - Display additions (+), modifications (~), deletions (-)
  - Color-coded output
  - Group by type (packages, files, services)

- [ ] `flux diff --generation <id>` - Compare against generation
  - Show what changed between current and specified generation
  - Useful for understanding history

- [ ] `flux diff <gen1> <gen2>` - Compare two generations
  - Show differences between any two states
  - Historical analysis

### Success Criteria

- [ ] Drift detection works for all resource types
- [ ] Status command shows comprehensive state
- [ ] Diff command accurately predicts changes
- [ ] Clear visual indicators (colors, symbols)
- [ ] Helpful suggestions for resolution

### Testing

- [ ] Test drift detection for packages
- [ ] Test drift detection for services
- [ ] Test drift detection for files
- [ ] Test status display
- [ ] Test diff accuracy

---

## Phase 6: Configuration Composition

**Goal**: Support modular, reusable configuration blocks

**Duration**: 2 weeks | **Status**: 🔲 Pending | **Priority**: MEDIUM

### Objectives

#### Include System (`src/config/mod.rs`)

- [ ] Add `include` field to config:

  ```toml
  [general]
  include = ["path/to/base.toml", "path/to/profile.toml"]
  ```

- [ ] Implement config file loading:
  - Load base config
  - Load each included config in order
  - Merge configurations (later overrides earlier)
  - Resolve relative paths
  - Detect circular includes

#### Profile Inheritance

- [ ] Add `inherits_from` field to profiles:

  ```toml
  [profiles.laptop]
  inherits_from = ["base", "mobile"]
  ```

- [ ] Implement inheritance resolution:
  - Build dependency graph
  - Topological sort for correct order
  - Merge profiles from base to specific
  - Override resolution rules

#### Conditional Configuration

- [ ] Add `enable_if` directive:

  ```toml
  [packages.tlp]
  version = "latest"
  enable_if = "hostname == 'laptop'"
  ```

- [ ] Supported conditions:
  - `hostname == "value"`
  - `env.VAR == "value"`
  - Platform checks (desktop, laptop, server)

#### Configuration Validation

- [ ] Validate include paths exist
- [ ] Detect circular dependencies
- [ ] Validate profile references
- [ ] Check condition syntax
- [ ] Warn about conflicts

### Success Criteria

- [ ] Includes work correctly
- [ ] Profile inheritance merges properly
- [ ] Conditionals evaluate correctly
- [ ] Circular dependencies detected
- [ ] Clear error messages for invalid configs

### Testing

- [ ] Test simple includes
- [ ] Test nested includes
- [ ] Test circular include detection
- [ ] Test profile inheritance
- [ ] Test conditional evaluation

---

## Phase 7: Package Operations Integration

**Goal**: Full package lifecycle management through declarative config

**Duration**: 2 weeks | **Status**: 🔲 Pending | **Priority**: MEDIUM

### Objectives

#### Package Operations

- [ ] Implement package installation in transaction:
  - Download packages before commit phase
  - Install atomically during commit
  - Rollback installations on failure
  - Track in generation

- [ ] Implement package removal in transaction:
  - Check dependencies before removal
  - Remove during commit phase
  - Track in generation

- [ ] Implement version constraint satisfaction:
  - Parse version constraints (~1.9, >=2.0, etc.)
  - Verify installed versions match
  - Upgrade if needed

#### Package Commands Enhancement

- [ ] `flux package install <name>` - Interactive install
  - Add to config
  - Install via transaction
  - Create generation
  
- [ ] `flux package remove <name>` - Interactive remove
  - Remove from config
  - Uninstall via transaction
  - Create generation
  
- [ ] `flux package upgrade <name>` - Upgrade to latest
  - Update version in config
  - Upgrade via transaction
  - Create generation
  
- [ ] `flux package pin <name> <version>` - Pin version
  - Update config with exact version
  - Downgrade/upgrade as needed

#### Dependency Management

- [ ] Query package dependencies
- [ ] Check for conflicts
- [ ] Warn before breaking changes
- [ ] Suggest resolution for conflicts

### Success Criteria

- [ ] Packages install/remove atomically
- [ ] Version constraints work correctly
- [ ] Dependencies are respected
- [ ] All operations tracked in generations
- [ ] Rollback restores packages correctly

### Testing

- [ ] Test package installation
- [ ] Test package removal
- [ ] Test version constraints
- [ ] Test dependency checking
- [ ] Test rollback of package changes

---

## Phase 8: Testing & Documentation

**Goal**: Production-ready with comprehensive tests and documentation

**Duration**: 2 weeks | **Status**: 🔲 Pending | **Priority**: HIGH

### Objectives

#### Testing Strategy

- [ ] **Unit Tests** for each service module:
  - Package manager operations
  - Service manager operations
  - Generation management
  - Transaction system
  - Configuration parsing
  - State verification

- [ ] **Integration Tests**:
  - Full `flux apply` workflow
  - Generation creation and rollback
  - Transaction commit and rollback
  - Multi-step operations

- [ ] **End-to-End Tests**:
  - Complete user workflows
  - Profile switching
  - Configuration composition
  - Error scenarios

- [ ] **Failure Scenario Tests**:
  - Network failures
  - Disk space issues
  - Permission errors
  - Corrupted state
  - Partial failures

#### Test Coverage Goals

- [ ] >80% code coverage for core modules
- [ ] 100% coverage for critical paths (transactions, generations)
- [ ] All error paths tested
- [ ] All commands have tests

#### Documentation

##### User Documentation

- [ ] **User Guide** (`docs/USER_GUIDE.md`):
  - Getting started tutorial
  - Common workflows
  - Configuration examples
  - Troubleshooting guide

- [ ] **Configuration Reference** (`docs/CONFIG_REFERENCE.md`):
  - All configuration options
  - Type specifications
  - Examples for each section
  - Best practices

- [ ] **Command Reference** (`docs/COMMAND_REFERENCE.md`):
  - All commands documented
  - Flags and options
  - Examples for each command
  - Output examples

##### Developer Documentation

- [ ] **Architecture Guide** (`docs/ARCHITECTURE.md`):
  - System overview
  - Component interactions
  - Data flow diagrams
  - Design decisions

- [ ] **Contributing Guide** (`CONTRIBUTING.md`):
  - Development setup
  - Code style guide
  - Testing requirements
  - PR process

- [ ] **API Documentation**:
  - All public APIs documented
  - Trait documentation
  - Usage examples
  - `cargo doc` generation

#### Migration Documentation

- [ ] **Migration Guide** (`docs/MIGRATION.md`):
  - Upgrading from old Flux
  - Converting imperative to declarative
  - Backward compatibility notes
  - Breaking changes (if any)

#### Error Handling

- [ ] Improve error messages throughout
- [ ] Add helpful suggestions for common errors
- [ ] Validate configurations before operations
- [ ] Graceful degradation when tools unavailable

### Success Criteria

- [ ] All tests passing
- [ ] >80% code coverage
- [ ] All commands documented
- [ ] User guide covers common workflows
- [ ] Developer documentation complete
- [ ] Migration guide available

### Testing Checklist

- [ ] Unit tests for all modules
- [ ] Integration tests for workflows
- [ ] E2E tests for user scenarios
- [ ] Failure scenario tests
- [ ] Performance tests
- [ ] Backward compatibility tests

---

## Future Enhancements (Post-MVP)

### Phase 9: Advanced Features (Optional)

#### System-Wide Package Management

- [ ] Support sudo-based system package management
- [ ] Multi-user machine support
- [ ] System service management

#### Container Management

- [ ] Declarative Podman/Docker containers
- [ ] Container lifecycle management
- [ ] Volume and network management

#### Desktop Environment Management

- [ ] GNOME/KDE/Sway configuration switching
- [ ] Theme management
- [ ] Extension management

#### Additional Features

- [ ] VPN/Network configuration
- [ ] Firewall rules management
- [ ] Cron jobs and systemd timers
- [ ] Font management
- [ ] Secrets management integration
- [ ] Cloud sync of generations
- [ ] Web UI for management
- [ ] Multi-distro support (Ubuntu, Arch)

---

## Success Metrics

### Technical Metrics

- **Reproducibility**: Same config = same state across machines
- **Atomicity**: Zero partial states in production use
- **Rollback Success**: 100% successful rollbacks in testing
- **Performance**: `flux apply` completes in <30 seconds typical
- **Test Coverage**: >80% for core modules

### User Metrics

- **Ease of Use**: New users productive within 15 minutes
- **Reliability**: No data loss incidents
- **Documentation**: <5 minutes to find answers in docs
- **Migration**: Existing users migrate without data loss

### Quality Metrics

- **Build Success**: All CI/CD checks pass
- **Code Quality**: No critical linter warnings
- **Dependencies**: All dependencies up to date and secure
- **Platform Support**: Works on all supported Fedora versions

---

## Dependencies & Prerequisites

### System Requirements

- **Fedora**: Version 38+ (for DNF and systemd)
- **Rust**: 1.70+ (for language features)
- **Git**: For configuration versioning
- **DNF**: For package management (optional, degrades gracefully)
- **Systemd**: For service management (optional, degrades gracefully)

### Rust Crates

- Core: `serde`, `toml`, `clap`, `colored`, `chrono`
- File ops: `walkdir`, `pathdiff`, `shellexpand`
- Git: `git2`
- Utils: `anyhow`, `thiserror`, `dirs`
- New (Phase 1+): `uuid`

---

## Risk Mitigation

### Technical Risks

**Risk**: State corruption during transaction

- **Mitigation**: Atomic operations, automatic rollback, temp directory staging
- **Severity**: High
- **Status**: Addressed in Phase 3

**Risk**: Generation storage grows unbounded

- **Mitigation**: Cleanup policies, configurable retention
- **Severity**: Medium
- **Status**: Addressed in Phase 2

**Risk**: Configuration merge conflicts

- **Mitigation**: Clear precedence rules, validation, conflict detection
- **Severity**: Medium
- **Status**: Addressed in Phase 6

### User Experience Risks

**Risk**: Users accidentally break system

- **Mitigation**: Dry-run by default, confirmation prompts, rollback capability
- **Severity**: High
- **Status**: Addressed throughout

**Risk**: Learning curve too steep

- **Mitigation**: Comprehensive documentation, examples, migration guide
- **Severity**: Medium
- **Status**: Addressed in Phase 8

**Risk**: Backward compatibility breaks

- **Mitigation**: Gradual rollout, feature flags, deprecation warnings
- **Severity**: High
- **Status**: Maintained throughout (Phase 1 proves concept)

---

## Timeline Summary

```
Weeks 1-2   ████ Phase 1: Foundation ✅ COMPLETE
Weeks 3-4   ░░░░ Phase 2: Generation System
Weeks 5-6   ░░░░ Phase 3: Atomic Transactions
Weeks 7-8   ░░░░ Phase 4: Declarative Apply
Weeks 9-10  ░░░░ Phase 5: State Verification
Weeks 11-12 ░░░░ Phase 6: Configuration Composition
Weeks 13-14 ░░░░ Phase 7: Package Operations
Weeks 15-16 ░░░░ Phase 8: Testing & Documentation
```

**Minimum Viable Product (MVP)**: Phases 1-5 (10 weeks)
**Feature Complete**: Phases 1-7 (14 weeks)
**Production Ready**: All Phases (16 weeks)

---

## How to Contribute

1. **Choose a Phase**: Pick an unstarted phase from the roadmap
2. **Review Design Docs**: Read relevant sections of `IMPLEMENTATION_GUIDE.md`
3. **Start Small**: Begin with a single feature or command
4. **Write Tests**: Add tests before implementation
5. **Document**: Update docs as you build
6. **Submit PR**: Follow `CONTRIBUTING.md` guidelines

---

## References

- **Design Documents**: `FEDORA_LAYER_VISION.md`, `NIXOS_COMPARISON.md`
- **Implementation**: `IMPLEMENTATION_GUIDE.md`, `FIRST_STEPS.md`
- **Phase 1**: `PHASE1_COMPLETE.md`, `PHASE1_QUICKSTART.md`
- **Documentation Index**: `DOCS_INDEX.md`

---

**Last Updated**: Phase 1 Complete (2024)
**Next Milestone**: Phase 2 - Generation System
**Target Completion**: 16 weeks from start of Phase 2
