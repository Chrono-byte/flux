# Application Icons

This directory should contain the application icons in various formats:

- `32x32.png` - 32x32 pixel PNG icon
- `128x128.png` - 128x128 pixel PNG icon
- `128x128@2x.png` - 256x256 pixel PNG icon (for high DPI)
- `icon.icns` - macOS icon bundle
- `icon.ico` - Windows icon file

## Generating Icons

You can generate these icons from a source SVG or high-resolution PNG using various tools:

### Using Tauri's icon generator

```bash
cargo install tauri-cli
cargo tauri icon path/to/icon.png
```

### Manual creation

1. Create a 1024x1024 PNG as the source
2. Use ImageMagick to generate the various sizes:

```bash
# PNG icons
convert icon-1024.png -resize 32x32 32x32.png
convert icon-1024.png -resize 128x128 128x128.png
convert icon-1024.png -resize 256x256 128x128@2x.png

# Windows ICO
convert icon-1024.png -define icon:auto-resize=256,128,64,48,32,16 icon.ico

# macOS ICNS (on macOS)
iconutil -c icns icon.iconset
```

## Placeholder

Until proper icons are created, Tauri will use default icons. The application will still build and run without custom icons.