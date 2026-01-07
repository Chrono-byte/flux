# Flux GUI (Tauri + Leptos)

A graphical frontend for the Flux dotfiles manager, built with [Tauri 2.0](https://tauri.app/) and [Leptos](https://leptos.dev/).

## Overview

This application provides a native desktop GUI for managing your dotfiles using the Flux CLI. It offers the same functionality as the CLI in an intuitive visual interface.

## Features

- **Dashboard**: Overview of sync status, git status, and quick actions
- **Files**: View and manage tracked dotfiles
- **Status**: Detailed sync status with statistics
- **Profiles**: Switch between dotfile profiles
- **Remotes**: Manage git remotes
- **History**: View operation history
- **Settings**: Configure dry-run mode and other options

## Prerequisites

Before building the application, ensure you have:

1. **Rust** (2024 edition) - Install via [rustup](https://rustup.rs/)
2. **Trunk** - Install via `cargo install trunk`
3. **wasm-bindgen-cli** - Install via `cargo install wasm-bindgen-cli`
4. **System dependencies for Tauri**:

   On Ubuntu/Debian:
   ```bash
   sudo apt update
   sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
   ```

   On Fedora:
   ```bash
   sudo dnf install webkit2gtk4.1-devel libappindicator-gtk3-devel librsvg2-devel
   ```

   On Arch:
   ```bash
   sudo pacman -S webkit2gtk-4.1 libappindicator-gtk3 librsvg
   ```

   On macOS:
   ```bash
   xcode-select --install
   ```

   On Windows:
   - Install [Microsoft Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   - Install [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/)

## Building

### Development Build

```bash
cd src-tauri
cargo build
```

### Release Build

```bash
cd src-tauri
cargo build --release
```

### Running in Development with Hot Reload

```bash
# In one terminal, start the Trunk dev server
cd src
trunk serve

# In another terminal, run Tauri
cd src-tauri
cargo tauri dev
```

Or use the combined command that runs both:

```bash
cd src-tauri
cargo tauri dev
```

This will start Trunk automatically and connect it to Tauri.

## Project Structure

```
flux-gui-tauri/
├── src/                    # Frontend (Leptos Rust + HTML, CSS)
│   ├── lib.rs             # Leptos application components
│   ├── main.rs            # Application entry point
│   ├── index.html         # Main HTML file
│   ├── index.js           # JS entry point
│   ├── styles.css         # Dark theme styles
│   └── Cargo.toml         # Rust dependencies
├── src-tauri/             # Backend (Rust)
│   ├── src/
│   │   ├── main.rs        # Application entry point
│   │   ├── commands.rs    # Tauri command handlers
│   │   └── state.rs       # Application state types
│   ├── Cargo.toml         # Tauri dependencies
│   ├── tauri.conf.json    # Tauri configuration
│   ├── Trunk.toml         # Trunk configuration
│   └── build.rs           # Build script
└── README.md              # This file
```

## Architecture

This application uses a Leptos-based frontend with Tauri:

- **Frontend**: Leptos (Rust web framework) that compiles to WebAssembly
- **Backend**: Rust code that interfaces with the Flux library
- **Build Tool**: Trunk (WASM bundler for Leptos)
- **IPC**: Tauri's `invoke` system for frontend-backend communication

### Key Components

1. **App State (`src/lib.rs`)**: Centralized reactive state management using Leptos signals

2. **Views (`src/lib.rs`)**: Leptos components for each view:
   - `HomeView`: Dashboard with sync status and quick actions
   - `FilesView`: List of tracked dotfiles
   - `StatusView`: Detailed sync statistics
   - `ProfilesView`: Profile management
   - `RemotesView`: Git remote operations
   - `HistoryView`: Operation history
   - `SettingsView`: Application settings

3. **Commands (`src-tauri/src/commands.rs`)**: Exposes Flux library operations as Tauri commands

4. **State (`src-tauri/src/state.rs`)**: Defines serializable types for IPC

## Comparison with Vanilla JS Version

This Leptos version replaces the original vanilla JS frontend:

| Feature | Vanilla JS | Leptos Version |
|---------|------------|----------------|
| UI Framework | Vanilla JS | Leptos (Rust) |
| State Management | Manual | Reactive signals |
| Type Safety | JSDoc | Full Rust types |
| Build Tool | None | Trunk |
| Hot Reload | Manual refresh | Built-in via Trunk |
| Bundle Size | Larger | Smaller (~600KB+) |

## Theming

The application uses a dark theme that matches the original GPUI version. Colors are defined as CSS variables in `styles.css`:

- Window background: `hsl(0, 0%, 12%)`
- Sidebar: `hsl(0, 0%, 10%)`
- Accent color: `hsl(166, 80%, 50%)` (cyan/teal)
- Status colors: Green (synced), Yellow (warning), Red (error)

## License

MIT License - See the main Flux repository for details.