# Changelog

All notable changes to Luma will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project uses semantic versioning once releases begin.

## [Unreleased]

## [0.1.1] - 2026-05-11

### Added

- Custom JSON palette definitions from `LUMA_THEME_DIR`, defaulting to `~/.config/luma/themes`.
- `lumactl theme validate` for one custom palette file or the active custom theme directory.
- JSON schema and example custom theme files.
- MIT SPDX license identifiers on source files.
- Dependabot configuration for Cargo and GitHub Actions update PRs.

### Changed

- `lumactl palettes` now lists built-in palettes plus valid custom palettes.
- Custom palettes override built-ins with the same key.
- CI now smoke-tests custom palette loading.
- CI supply-chain checks use pinned GitHub Actions, `cargo audit`, and `cargo deny`.
- Library crate entrypoints are thin module/export maps; implementations live in named modules.

## [0.1.0] - 2026-05-11

### Added

- macOS appearance backend using native appearance change notifications.
- `lumactl` CLI with `install`, `sync`, `watch`, `toggle`, `dark`, `light`, `status`, `config`, `palettes`, `plugins`, and `uninstall` commands.
- Built-in plugins for Nvim, Ghostty, tmux, K9s, and Pi.
- Generic-safe tmux palette mode plus opt-in statusline mode.
- Built-in Nightfox-family palettes for generated themes.
