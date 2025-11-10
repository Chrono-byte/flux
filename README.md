# Dotfiles Manager

A powerful, symlink-based dotfiles manager written in Rust. Manage your configuration files across multiple machines and profiles with ease.

## Features

- **Symlink-based sync**: Files stored in repository, symlinked to home directory
- **Profile support**: Multiple profiles with per-file overrides
- **Browser integration**: Auto-detect and backup Firefox and Zen browser settings
- **Git integration**: Automatic commit with user-prompted messages
- **File locking detection**: Skips locked files with warnings (using `flock`)
- **Dry-run mode**: Preview changes before applying
- **Status checking**: See sync status of all tracked files
- **Backup & restore**: Automatic backups with restore capability
- **Validation**: Check configuration integrity

## Installation

```bash
git clone <repository-url>
cd dotfiles-manager
cargo build --release
sudo cp target/release/dotfiles-manager /usr/local/bin/
```

## Quick Start

1. **Initialize repository**:
   ```bash
   dotfiles-manager init
   ```

2. **Add a file**:
   ```bash
   dotfiles-manager add sway ~/.config/sway/config --dest .config/sway/config
   ```

3. **Sync files**:
   ```bash
   dotfiles-manager sync
   ```

4. **Check status**:
   ```bash
   dotfiles-manager status
   ```

## Commands

### `init [--repo-path PATH]`
Initialize dotfiles repository. Creates config directory and sets up repository structure.

### `add <tool> <file> [--dest PATH] [--profile NAME]`
Add a file to tracking under a tool section.

**Examples**:
```bash
dotfiles-manager add sway ~/.config/sway/config
dotfiles-manager add cursor ~/.config/Cursor/User/settings.json --dest .config/Cursor/User/settings.json
```

### `add-browser [browser]`
Auto-detect and add browser profiles (Firefox and Zen). Defaults to `all`.

**Examples**:
```bash
dotfiles-manager add-browser
dotfiles-manager add-browser firefox
dotfiles-manager add-browser zen
```

### `sync [--profile NAME] [--dry-run]`
Sync tracked files (create symlinks). Use `--dry-run` to preview changes.

**Examples**:
```bash
dotfiles-manager sync
dotfiles-manager sync --profile work
dotfiles-manager sync --dry-run
```

### `status [--profile NAME]`
Show sync status of all tracked files.

### `list [--profile NAME]`
List all tracked files.

### `remove <tool> <file>`
Remove a file from tracking.

### `restore [backup] [--file PATH]`
Restore files from backup. Use `latest` or backup index number.

**Examples**:
```bash
dotfiles-manager restore latest
dotfiles-manager restore 1
dotfiles-manager restore latest --file ~/.config/sway/config
```

### `validate`
Validate configuration integrity. Checks for missing files, broken symlinks, and orphaned entries.

### `profile create <name>`
Create a new profile.

### `profile switch <name>`
Switch to a different profile.

### `profile list`
List all available profiles.

## Configuration

Configuration is stored in `~/.config/dotfiles-manager/config.toml`.

### Example Configuration

```toml
[general]
repo_path = "~/.dotfiles"
current_profile = "default"
backup_dir = "~/.dotfiles/.backups"
symlink_resolution = "auto"  # auto, relative, absolute, follow, replace

[tools.sway]
files = [
    { repo = "config", dest = ".config/sway/config" },
    { repo = "config.work", dest = ".config/sway/config", profile = "work" }
]

[tools.cursor]
files = [
    { repo = "settings.json", dest = ".config/Cursor/User/settings.json" }
]

[tools.firefox]
files = [
    { repo = "prefs.js", dest = ".mozilla/firefox/xxxxx.default/prefs.js" },
    { repo = "extensions", dest = ".mozilla/firefox/xxxxx.default/extensions" }
]
```

### Symlink Resolution Options

- `auto`: Use relative if possible, absolute if needed (default)
- `relative`: Always create relative symlinks
- `absolute`: Always create absolute symlinks
- `follow`: Follow existing symlinks, replace target
- `replace`: Replace symlinks with actual files (copy)

## Browser Support

### Firefox
Automatically detects default Firefox profile and backs up:
- `prefs.js` - Preferences
- `user.js` - User overrides
- `places.sqlite` - Bookmarks and history
- `extensions/` - Installed extensions
- `storage/` - Extension storage

### Zen Browser
Automatically detects default Zen profile and backs up the same files.

**Usage**:
```bash
dotfiles-manager add-browser
dotfiles-manager sync
```

## Profiles

Profiles allow you to have different configurations for different machines or use cases.

### Creating a Profile

```bash
dotfiles-manager profile create work
```

### Profile Overrides

Profile-specific files override base files for the same destination. Base files are used if not overridden.

**Example**:
- Base: `sway/config` → `.config/sway/config`
- Profile "work": `profiles/work/sway/config` → `.config/sway/config` (overrides base)

## File Locking

The tool uses `flock` to detect locked files. If a file is locked (e.g., browser is running), it will be skipped with a warning:

```
⚠ Warning: /path/to/file is locked (may be in use), skipping
```

## Backups

Backups are automatically created in `~/.dotfiles/.backups/` when conflicts are resolved. Each backup is timestamped.

**Restore from backup**:
```bash
dotfiles-manager restore latest
dotfiles-manager restore 1 --file ~/.config/sway/config
```

## Git Integration

The tool automatically initializes a git repository and commits changes after sync operations. You'll be prompted for commit messages per changed file.

## Examples

### Setting up Sway WM

```bash
# Initialize
dotfiles-manager init

# Add sway config
dotfiles-manager add sway ~/.config/sway/config

# Sync
dotfiles-manager sync
```

### Managing Browser Settings

```bash
# Auto-detect and add browser profiles
dotfiles-manager add-browser

# Sync (will skip if browser is running)
dotfiles-manager sync

# Check status
dotfiles-manager status
```

### Using Profiles

```bash
# Create work profile
dotfiles-manager profile create work

# Add work-specific config
dotfiles-manager add sway ~/.config/sway/config.work --profile work --dest .config/sway/config

# Switch to work profile
dotfiles-manager profile switch work

# Sync work profile
dotfiles-manager sync
```

## Troubleshooting

### Files are being skipped
- Check if files are locked (browser/application running)
- Use `status` command to see details
- Close applications and try again

### Symlinks not working
- Check `symlink_resolution` in config
- Use `validate` to check for issues
- Ensure repository path is correct

### Profile not working
- Verify profile exists: `dotfiles-manager profile list`
- Check profile directory exists in repository
- Use `validate` to check configuration

## License

[Your License Here]

## Contributing

[Contributing Guidelines]

