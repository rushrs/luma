# Security Policy

## Supported versions

Luma is pre-1.0. Security fixes are made on the main branch until formal release branches exist.

## Reporting a vulnerability

Please open a private security advisory on GitHub, or contact the maintainers through the repository owner profile.

Do not include secrets, tokens, private keys, or passwords in public issues. If a report requires logs, redact sensitive values first.

## Security model

Luma is a local developer-tool coordinator. It writes theme/config files under the current user's home directory and, on macOS, can install a user LaunchAgent for `lumactl watch`.

Expected boundaries:

- Luma should not require root privileges.
- Luma should not read or transmit secrets.
- Luma should only modify documented app config/theme files and managed blocks.
- `lumactl uninstall` should remove only Luma-managed files/blocks and leave user config otherwise intact.

## Dependency checks

The CI workflow runs Rust formatting, clippy, tests, and a RustSec audit. Contributors should run the same locally before release.
