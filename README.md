# Flux

A powerful, symlink-based dotfiles manager written in Rust. Manage your
configuration files across multiple machines and profiles with ease.

## Features

- **Symlink-based sync**: Files stored in repository, symlinked to home
  directory
- **Profile support**: Multiple profiles with per-file overrides
- **Browser integration**: Auto-detect and backup Firefox and Zen browser
  settings
- **Git integration**: Automatic commit with user-prompted messages, remote
  management, and push support
- **Remote management**: Add, remove, and manage git remotes (GitHub, GitLab,
  Gitea, etc.)
- **Push to remote**: Push your dotfiles with support for SSH and HTTPS
  authentication
- **File locking detection**: Skips locked files with warnings (using `flock`)
- **Dry-run mode**: Preview changes before applying
- **Status checking**: See sync status of all tracked files
- **Backup & restore**: Automatic backups with restore capability
- **Validation**: Check configuration integrity

## Installation

```bash
git clone <repository-url>
cd flux
cargo build --release
sudo cp target/release/flux /usr/local/bin/
```

## Quick Start

1. **Initialize repository**:

   ```bash
   flux init
   ```

2. **Add a file**:

   ```bash
   flux add sway ~/.config/sway/config --dest .config/sway/config
   ```

3. **Sync files**:

   ```bash
   flux sync
   ```

4. **Check status**:

   ```bash
   flux status
   ```

## Commands

### `init [--repo-path PATH]`

Initialize dotfiles repository. Creates config directory and sets up repository
structure.

### `add <tool> <file> [--dest PATH] [--profile NAME]`

Add a file to tracking under a tool section.

**Examples**:

```bash
flux add sway ~/.config/sway/config
flux add cursor ~/.config/Cursor/User/settings.json --dest .config/Cursor/User/settings.json
```

### `add-browser [browser]`

Auto-detect and add browser profiles (Firefox and Zen). Defaults to `all`.

**Examples**:

```bash
flux add-browser
flux add-browser firefox
flux add-browser zen
```

### `sync [--profile NAME] [--dry-run]`

Sync tracked files (create symlinks). Use `--dry-run` to preview changes.

**Examples**:

```bash
flux sync
flux sync --profile work
flux sync --dry-run
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
flux restore latest
flux restore 1
flux restore latest --file ~/.config/sway/config
```

### `validate`

Validate configuration integrity. Checks for missing files, broken symlinks, and
orphaned entries.

### `profile create <name>`

Create a new profile.

### `profile switch <name>`

Switch to a different profile.

### `profile list`

List all available profiles.

## Configuration

Configuration can be stored in one of several locations, checked in this order:

1. **Environment variable** - Set `DOTFILES_CONFIG=/path/to/config.toml`
2. **Repository** - `~/.dotfiles/config.toml` (allows keeping config with dotfiles)
3. **System config** - `~/.config/flux/config.toml` (XDG standard location)

The first found configuration file is used. This means you can optionally keep your
configuration file in your dotfiles repository for easier portability and version control.

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
flux add-browser
flux sync
```

## Profiles

Profiles allow you to have different configurations for different machines or
use cases.

### Creating a Profile

```bash
flux profile create work
```

### Profile Overrides

Profile-specific files override base files for the same destination. Base files
are used if not overridden.

**Example**:

- Base: `sway/config` → `.config/sway/config`
- Profile "work": `profiles/work/sway/config` → `.config/sway/config` (overrides
  base)

## File Locking

The tool uses `flock` to detect locked files. If a file is locked (e.g., browser
is running), it will be skipped with a warning:

```md
⚠ Warning: /path/to/file is locked (may be in use), skipping
```

## Backups

Backups are automatically created in `~/.dotfiles/.backups/` when conflicts are
resolved. Each backup is timestamped.

**Restore from backup**:

```bash
flux restore latest
flux restore 1 --file ~/.config/sway/config
```

## Git Integration

The tool automatically initializes a git repository and commits changes after
sync operations. You'll be prompted for commit messages per changed file.

### Remote Management and Pushing

You can manage git remotes and push your dotfiles to GitHub, GitLab, Gitea, or
any other git hosting service.

#### Add a Remote

```bash
# Add origin remote (SSH)
flux remote add origin git@github.com:username/dotfiles.git

# Add origin remote (HTTPS)
flux remote add origin https://github.com/username/dotfiles.git

# Add with dry-run to preview
flux remote add origin git@github.com:username/dotfiles.git --dry-run
```

#### Remove a Remote

```bash
flux remote remove origin
flux remote remove upstream --dry-run
```

#### Change Remote URL

```bash
flux remote set-url origin git@github.com:username/dotfiles.git
flux remote set-url origin https://github.com/username/dotfiles.git
```

#### List Remotes

```bash
flux remote list
```

#### Push to Remote

Push your dotfiles to a remote repository:

```bash
# Push with default settings (origin, current branch)
flux push

# Push to specific remote
flux push --remote upstream

# Push specific branch
flux push --branch main

# Set upstream branch after push
flux push --set-upstream

# Preview with dry-run
flux push --dry-run

# Combined options
flux push --remote origin --branch main --set-upstream
```

#### Default Remote and Branch Configuration

You can set default remote and branch in your config to avoid repeated flags:

```toml
[general]
# ... other settings ...
default_remote = "origin"
default_branch = "main"
```

#### GitHub Setup

1. Create a repository on GitHub
2. Add SSH key to GitHub (or use HTTPS with PAT)
3. Add remote:

   ```bash
   flux remote add origin git@github.com:username/dotfiles.git
   ```

4. Push:

   ```bash
   flux push --set-upstream
   ```

#### GitLab Setup

Similar to GitHub, but use GitLab's git URLs:

```bash
flux remote add origin git@gitlab.com:username/dotfiles.git
flux push --set-upstream
```

#### Gitea Setup

For self-hosted Gitea instances:

```bash
flux remote add origin git@gitea.example.com:username/dotfiles.git
flux push --set-upstream
```

#### Authentication

The tool supports two authentication methods:

**SSH** (Recommended):

- Uses SSH agent automatically
- Ensure your SSH key is added to the agent: `ssh-add ~/.ssh/id_ed25519`

**HTTPS with Personal Access Token**:

- Set environment variables:

  ```bash
  export GIT_USERNAME=your_username
  export GIT_PASSWORD=your_personal_access_token
  flux push
  ```

## Examples

### Setting up Sway WM

```bash
# Initialize
flux init

# Add sway config
flux add sway ~/.config/sway/config

# Sync
flux sync
```

### Managing Browser Settings

```bash
# Auto-detect and add browser profiles
flux add-browser

# Sync (will skip if browser is running)
flux sync

# Check status
flux status
```

### Using Profiles

```bash
# Create work profile
flux profile create work

# Add work-specific config
flux add sway ~/.config/sway/config.work --profile work --dest .config/sway/config

# Switch to work profile
flux profile switch work

# Sync work profile
flux sync
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

- Verify profile exists: `flux profile list`
- Check profile directory exists in repository
- Use `validate` to check configuration

## Versioning

Flux uses **Epoch Semantic Versioning**, an extension of Semantic Versioning that provides better communication about the scale of changes. For detailed information about our versioning scheme, see [VERSIONING.md](VERSIONING.md).

## License

[Your License Here]

## Contributing

[Contributing Guidelines]
