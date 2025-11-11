# Flux Versioning Scheme: Epoch Semantic Versioning

As of v1.0.0, Flux adopts **Epoch Semantic Versioning** (Epoch SemVer), an extension of [Semantic Versioning](https://semver.org/) inspired by [Anthony Fu's proposal](https://antfu.me/posts/epoch-semver).

## Overview

Epoch SemVer extends the traditional `MAJOR.MINOR.PATCH` format to provide more granular communication about the scale of changes:

```md
{EPOCH * 1000 + MAJOR}.MINOR.PATCH
```

## Version Components

### EPOCH (Encoded in MAJOR)

- **Meaning**: Significant, groundbreaking changes or major versions
- **Increment**: When making fundamental architectural changes or major milestone releases
- **Range**: 0-∞ (multiplied by 1000 and encoded in the first number)
- **Example**: EPOCH 0 → first number 0-999, EPOCH 1 → first number 1000-1999

### MAJOR (Encoded in MAJOR)

- **Meaning**: Minor incompatible API changes with technical significance
- **Increment**: When making breaking changes that may not be significant to all users
- **Range**: 0-999 (modulo within each EPOCH)
- **Compatibility**: Breaking changes that require code updates

### MINOR

- **Meaning**: Backwards-compatible feature additions
- **Increment**: When adding new functionality without breaking existing APIs
- **Compatibility**: Safe to upgrade

### PATCH

- **Meaning**: Backwards-compatible bug fixes
- **Increment**: When fixing bugs without changing functionality
- **Compatibility**: Safe to upgrade

## Flux Versioning History

### Pre-1.0.0

- **v0.1.0**: Initial development release
  - Baseline feature set: core dotfile management
  - Early feature exploration phase

### v1.0.0 and Beyond

- **v1.0.0**: First stable production release (EPOCH=0, MAJOR=1, MINOR=0, PATCH=0)
  - Marks project as production-ready
  - All v1.x versions maintain core API stability

## Version Format Examples

| Version | Meaning |
|---------|---------|
| `1.0.0` | Initial stable release (EPOCH 0, MAJOR 1) |
| `1.5.3` | Minor features + patches (same EPOCH & MAJOR) |
| `2.0.0` | Breaking API change (EPOCH 0, MAJOR 2) |
| `1000.0.0` | Major milestone/rewrite (EPOCH 1, MAJOR 0) |
| `1001.5.2` | EPOCH 1 with features and patches (EPOCH 1, MAJOR 1) |
| `2000.0.0` | Second major era (EPOCH 2, MAJOR 0) |

## Incrementing Versions

### When to bump MAJOR (within current EPOCH)

Use for:

- Removing or renaming commands
- Breaking configuration format changes
- Changes affecting a significant subset of users (even if not 99.9%)

Example: `1.0.0` → `2.0.0`

### When to bump MINOR

Use for:

- New commands or subcommands
- New configuration options (backwards-compatible)
- New features in existing functionality

Example: `1.0.0` → `1.1.0`

### When to bump PATCH

Use for:

- Bug fixes
- Performance improvements
- Documentation updates

Example: `1.0.0` → `1.0.1`

### When to bump EPOCH

Use for:

- Complete architectural rewrites
- Fundamental shifts in how the tool operates
- Major milestone releases warranting a codename

Example: `1999.x.x` → `1000.0.0` (EPOCH 1)

This is **rare** and typically accompanied by a major marketing effort.

## Changelog and Release Notes

Every release should include:

1. **Version number** in Epoch SemVer format
2. **Date** of release
3. **Changes** categorized as:
   - ✨ Features
   - 🐛 Bug Fixes
   - ⚠️ Breaking Changes
   - 📚 Documentation
   - 🔧 Internal

Example:

```markdown
## [1.1.0] - 2025-11-15

### ✨ Features
- Add support for multiple symlink strategies

### 🐛 Bug Fixes
- Fix file locking detection on NFS mounts

### 📚 Documentation
- Update profile configuration examples
```

## Migration from v0.1.0

The transition from `v0.1.0` to `v1.0.0` represents:

- Recognition of production-readiness
- Stabilization of core APIs
- Commitment to versioning stability using Epoch SemVer

**No breaking changes** were made in this transition—it's purely a versioning scheme adoption.

## References

- [Semantic Versioning 2.0.0](https://semver.org/)
- [Epoch Semantic Versioning (Anthony Fu)](https://antfu.me/posts/epoch-semver)
- [Debian Versioning Policy](https://www.debian.org/doc/debian-policy/ch-controlfields.html#Version)
