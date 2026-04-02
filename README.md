# Flux

A symlink-based dotfiles manager written in Rust. Manage your configuration files across multiple machines and profiles with ease.

## Features

- **Symlink-based sync**: Files stored in repository, symlinked to home directory
- **Profile support**: Multiple profiles with per-file overrides
- **Git integration**: Automatic commits, remote management, and push support (SSH/HTTPS)
- **File locking detection**: Skips locked files with warnings
- **Dry-run mode**: Preview changes before applying
- **Transaction safety**: Automatic backups during apply operations for rollback safety
- **Validation**: Check configuration integrity

## Installation

### From Source

```bash
git clone <repository-url>
cd flux
cargo install --path .
```

Installs to `~/.cargo/bin/flux`. Ensure `~/.cargo/bin` is in your PATH.

### Alternatives

**From Git:**

```bash
cargo install --git <repository-url>
```

## Quick Start

```bash
# Initialize repository
flux init

# Add a file
flux add sway ~/.config/sway/config --dest .config/sway/config

# Commit files (sync and create symlinks)
flux commit

# Check status
flux status
```

## Commands

### File Management

- `flux add <tool> <file> [--dest PATH] [--profile NAME] [--from-repo]` - Add file to tracking (use `--from-repo` to register a file that already exists in repo without copying)
- `flux commit [--profile NAME] [--message MSG] [--dry-run] [--verbose]` - Sync tracked files (create symlinks) and commit changes. Use `--verbose` to show detailed progress for each file.
- `flux rm <tool> <file> [--dry-run]` - Remove file from tracking
- `flux ls-files [--profile NAME]` - List all tracked files (alias: `flux list`)
- `flux status [--profile NAME]` - Show sync status of all tracked files

### Profiles

- `flux profile list` - List all profiles
- `flux profile create <name>` - Create a new profile
- `flux profile switch <name>` - Switch to a different profile

### Configuration Management Commands

- `flux config sync [--dry-run]` - Sync XDG config to repo

### Apply Configuration

- `flux apply [--profile NAME] [--dry-run] [--yes] [--force]` - Apply tracked files to their destinations, creating symlinks. Use `--force` to replace all files that aren't correct symlinks without creating backups

### Git Operations

- `flux remote list` - List remotes
- `flux remote add <name> <url>` - Add remote
- `flux remote remove <name>` - Remove remote
- `flux remote set-url <name> <url>` - Change remote URL
- `flux push [--remote NAME] [--branch NAME] [--set-upstream]` - Push to remote
- `flux pull [--remote NAME] [--branch NAME]` - Pull from remote

- `flux diff [--profile NAME] [--force] [--name-only]` - Show git-style diffs between repository files and their destination paths. The command first lists files that need syncing and then displays unified diffs for each change; use `--name-only` to print just the paths, `--force` to treat every tracked file as out-of-sync (same as `flux apply --force`), and note that diffs are piped through `$PAGER` (default `less -R`) when stdout is a terminal.

- `flux doctor [--profile NAME]` - Run validation, state diff, and health checks; exits non-zero (1=validation, 2=diff, 4=health, bits can combine)
- `flux maintain migrate [--profile NAME] [--no-backup]` - Migrate files with discrepancies (use `--no-backup` to skip copying files to repo, just remove and create symlinks)
- `flux maintain gitignore` - Generate a .gitignore file for the repo

### Completions

- `flux completion <shell>` - Generate shell completions (zsh, bash, fish, etc.)

## Configuration

Configuration is checked in this order:

1. Environment variable: `DOTFILES_CONFIG=/path/to/config.toml`
2. Repository: `~/.dotfiles/config.toml`
3. System config: `~/.config/flux/config.toml` (XDG standard)

The first found file is used.

### Example Configuration

```toml
[general]
repo_path = "~/.dotfiles"
current_profile = "default"
# backup_dir is optional - defaults to ~/.local/share/flux/backups if not specified
symlink_resolution = "auto"  # auto, relative, absolute, follow, replace
default_remote = "origin"
default_branch = "main"

[tools.sway]
files = [
    { repo = "config", dest = ".config/sway/config" },
    { repo = "config.work", dest = ".config/sway/config", profile = "work" }
]

[tools.cursor]
files = [
    { repo = "settings.json", dest = ".config/Cursor/User/settings.json" }
]
```

### Repository Paths

File paths in the `repo` field support the following patterns:

- `filename` - File in the tool's directory (e.g., `config` becomes `~/.dotfiles/sway/config`)
- `subdir/filename` - File in a subdirectory (e.g., `config.d/binding.conf` becomes `~/.dotfiles/sway/config.d/binding.conf`)
- `./filename` - File at repository root, not in tool directory (e.g., `./config.toml` becomes `~/.dotfiles/config.toml`)
- `toolname/filename` - Explicit tool prefix (e.g., `cursor/settings.json` becomes `~/.dotfiles/cursor/settings.json`)

The `./` prefix is useful for configuration files that should live at the repository root rather than in a tool-specific directory.

## Flux Configuration

Flux's own configuration (`~/.dotfiles/config.toml`) is the source of truth and not tracked as a managed file. To ensure Flux always uses your repository configuration, set the environment variable:

```bash
export DOTFILES_CONFIG="$HOME/.dotfiles/config.toml"
```

Add this to your shell config (e.g., `~/.zshrc`) so Flux always reads from your repository config, not from the XDG location.

### Symlink Resolution

- `auto` - Use relative if possible, absolute if needed (default)
- `relative` - Always create relative symlinks
- `absolute` - Always create absolute symlinks
- `follow` - Follow existing symlinks, replace target
- `replace` - Replace symlinks with actual files (copy)

## Profiles Commands

Profiles allow different configurations for different machines or use cases. Profile-specific files override base files for the same destination.

```bash
flux profile create work
flux add sway ~/.config/sway/config.work --profile work --dest .config/sway/config
flux profile switch work
flux commit
```

## Git Integration

Flux automatically initializes a git repository and commits changes after sync operations.

### Remote Setup

```bash
# Add remote (SSH recommended)
flux remote add origin git@github.com:username/dotfiles.git

# Pull from remote
flux pull

# Push to remote
flux push --set-upstream

# Or set defaults in config
[general]
default_remote = "origin"
default_branch = "main"
```

### Authentication

**SSH** (recommended): Uses SSH agent automatically. Ensure your key is added:

```bash
ssh-add ~/.ssh/id_ed25519
```

**HTTPS**: Set environment variables:

```bash
export GIT_USERNAME=your_username
export GIT_PASSWORD=your_personal_access_token
flux push
```

## Examples

### Basic Setup

```bash
flux init
flux add sway ~/.config/sway/config
flux commit
```

### How to use Profiles

```bash
flux profile create work
flux add sway ~/.config/sway/config.work --profile work --dest .config/sway/config
flux profile switch work
flux commit
```

### Declarative Configuration

```bash
# Apply files
flux apply

# Preview changes
flux apply --dry-run
```

## Troubleshooting

**Files being skipped**: Check if files are locked (browser/application running). Use `flux status` for details.

**Symlinks not working**: Check `symlink_resolution` in config. Use `flux doctor --profile <name>` to validate your configuration and system state.

**Profile not working**: Verify with `flux profile list`. Check profile directory exists in repository.

## Versioning

Flux uses **Epoch Semantic Versioning**. See [VERSIONING.md](VERSIONING.md) for details.
