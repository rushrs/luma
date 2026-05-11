# Contributing

Thanks for considering a contribution to Luma.

## Development setup

```bash
cargo fmt
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo audit --deny warnings
cargo deny check all
```

For local install testing without touching real app config:

```bash
TMP_HOME="$(mktemp -d)"
HOME="$TMP_HOME" LUMA_NO_LIVE=1 cargo run -p lumactl -- install
```

## Design principles

- Keep app adapters plugin-based and selected by config.
- Keep plugins as OS-agnostic as practical; OS-specific paths and appearance behavior belong in `Platform` / `AppearanceBackend` crates.
- Prefer generic-safe defaults. Opinionated integrations should be opt-in.
- Do not add personal paths, private service names, or machine-specific assumptions to default behavior.
- Do not log or commit secrets.
- Keep `lib.rs` files as small module/export maps; implementation code should live in named modules such as `nvim.rs`, `k9s.rs`, or `macos.rs`.

## Adding a plugin

1. Add the app identifier/path behavior to the platform crate if needed.
2. Implement `LumaPlugin` in the relevant plugin crate.
3. Add the plugin name to `lumactl` selection.
4. Document managed files and config options in `README.md`.
5. Add tests where behavior can be tested without touching real user files.

## Pull request checklist

- [ ] `cargo fmt --check`
- [ ] `cargo clippy --locked --all-targets --all-features -- -D warnings`
- [ ] `cargo test --locked --all-features`
- [ ] `cargo audit --deny warnings`
- [ ] `cargo deny check all`
- [ ] no personal paths or secrets in the diff
- [ ] README/config docs updated if behavior changed
