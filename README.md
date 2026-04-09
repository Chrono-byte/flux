# Flux

A symlink-based dotfiles manager written in Rust. Manage your configuration files across multiple machines and profiles with ease.

## Shell Init Check

If you want a startup-time check that stays quiet when the remote cannot be reached, add this to your shell init file:

```sh
flux remote check
```

It only prints a notice when the Flux repo is not at the current remote head. If the machine is offline or the remote cannot be queried, it exits quietly.

## Versioning

Flux uses **Epoch Semantic Versioning**. See [VERSIONING.md](VERSIONING.md) for details.
