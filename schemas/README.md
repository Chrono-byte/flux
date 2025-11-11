# Flux Configuration Schema

This directory contains JSON schema files for Flux configuration files, enabling editor autocomplete, validation, and documentation.

## Schema File

- `config.schema.json` - JSON schema for `config.toml` configuration files

## Using the Schema

### Method 1: Taplo Configuration File (Easiest)

Create a `taplo.toml` file in your project root or in `~/.config/taplo/taplo.toml`:

```toml
[schema]
enabled = true
path = "./schemas/config.schema.json"

[[rule]]
name = "flux-config"
include = [
  "**/config.toml",
  "**/flux/config.toml",
  "~/.config/flux/config.toml",
  "~/.dotfiles/config.toml"
]
schema = { enabled = true, path = "./schemas/config.schema.json" }
```

If you're using this project, a `taplo.toml` file is already included in the repository root.

### Method 2: Editor Configuration

Configure your editor to automatically use the schema for config files.

### VS Code / Cursor

1. Install the [Even Better TOML](https://marketplace.visualstudio.com/items?itemName=tamasfe.even-better-toml) extension
2. Add to your workspace settings (`.vscode/settings.json`):

```json
{
  "evenBetterToml.schema.associations": {
    "**/config.toml": "./schemas/config.schema.json",
    "~/.config/flux/config.toml": "./schemas/config.schema.json",
    "~/.dotfiles/config.toml": "./schemas/config.schema.json"
  }
}
```

Or add to your user settings for global use:

```json
{
  "evenBetterToml.schema.associations": {
    "**/flux/config.toml": "/path/to/dotfiles-manager/schemas/config.schema.json",
    "**/.dotfiles/config.toml": "/path/to/dotfiles-manager/schemas/config.schema.json"
  }
}
```

### Neovim / Vim

Using [nvim-lspconfig](https://github.com/neovim/nvim-lspconfig) with `taplo`:

1. Install `taplo` LSP server
2. Configure schema in your config:

```lua
require('lspconfig').taplo.setup {
  settings = {
    taplo = {
      schemas = {
        {
          fileMatch = { "**/flux/config.toml", "**/.dotfiles/config.toml" },
          url = "file:///path/to/dotfiles-manager/schemas/config.schema.json"
        }
      }
    }
  }
}
```

### Method 3: Inline Schema Reference (If Supported)

Some editors support inline schema references. You can add this comment at the top of your `config.toml`:

```toml
# yaml-language-server: $schema=./schemas/config.schema.json
# or for taplo:
# schema = "./schemas/config.schema.json"
```

Note: This depends on your editor's TOML language server support.

### Other Editors

Most modern editors support JSON schemas for TOML files. Check your editor's documentation for:
- TOML language server support
- JSON schema association for TOML files
- Configuration file validation

## Schema Features

The schema provides:

- **Autocomplete**: Type hints and suggestions for all configuration options
- **Validation**: Real-time error checking for invalid values
- **Documentation**: Inline descriptions and examples for each field
- **Type Safety**: Enforced types and patterns (e.g., profile names must be alphanumeric)

## Example Usage

Once configured, when editing `config.toml`, you'll get:

- Autocomplete suggestions when typing `[general.`
- Validation errors for invalid enum values (e.g., `symlink_resolution = "invalid"`)
- Hover documentation explaining each field
- Format validation for profile names, paths, etc.

## Updating the Schema

If the configuration format changes, update `config.schema.json` to match the new structure. The schema follows JSON Schema Draft 7 specification.

