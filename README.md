# Luma / `lumactl`

Luma coordinates light/dark color schemes across local developer tools. The CLI is `lumactl`.

Built-in plugin adapters currently cover:

- `nvim` — writes `~/.cache/luma/mode` and `~/.cache/luma/nvim-colorscheme`; optional Lua integration watches those files
- `ghostty` — writes `theme = light:...,dark:...`
- `tmux` — generates `~/.tmux/luma.tmux.conf`, sources it from `~/.tmux.conf`, and live-sources it into running tmux when available. Defaults to generic-safe palette variables; optional statusline mode owns the tmux bar.
- `k9s` — generates `~/Library/Application Support/k9s/skins/luma.yaml` and selects it. Primary foreground/background use terminal defaults so main text/background recolors immediately when the terminal theme changes; K9s accent colors still reload through K9s' reactive skin watcher.
- `pi` — generates `~/.pi/agent/themes/luma.json` and selects it in `~/.pi/agent/settings.json`

## Fresh macOS install

Prerequisites:

```bash
xcode-select --install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh # if Rust is missing
```

Install with defaults from a checkout:

```bash
git clone https://github.com/rushrs/luma.git
cd luma
./install.sh
```

Choose schemes and plugins:

```bash
./install.sh --light dawnfox --dark carbonfox --plugins nvim,ghostty,tmux,k9s,pi
./install.sh --light dayfox --dark nightfox --plugins nvim,ghostty,tmux,k9s
./install.sh --light dawnfox --dark duskfox --plugins nvim,pi
```

## Config

Luma config lives at:

```text
~/.config/luma/config
```

Example:

```sh
LUMA_LIGHT=dawnfox
LUMA_DARK=carbonfox
LUMA_PLUGINS=nvim,ghostty,tmux,k9s,pi
LUMA_TMUX_MODE=palette
```

Tmux modes:

```sh
# Generic-safe: only sets @luma_* / @luma_tmux_* color variables.
LUMA_TMUX_MODE=palette

# Opinionated: Luma owns status-left/status-right/window-status formats.
LUMA_TMUX_MODE=statusline

# No tmux UI management.
LUMA_TMUX_MODE=off
```

If a terminal uses a display name different from the canonical scheme key:

```sh
LUMA_LIGHT_GHOSTTY=Dawnfox
LUMA_DARK_GHOSTTY=Carbonfox
```

The light/dark scheme keys are selected in config. Built-in palette color values are defined in Rust in:

```text
crates/luma-core/src/lib.rs  # PALETTES
```

Plugins that need concrete generated colors, like K9s and Pi, use those palette definitions. Plugins that natively support theme names, like Nvim and Ghostty, use the configured scheme names/display names directly.

## Commands

```bash
lumactl sync
lumactl toggle
lumactl dark
lumactl light
lumactl status
lumactl config --show
lumactl config --light dayfox --dark nightfox --plugins nvim,ghostty,tmux,k9s
lumactl config --tmux-mode palette
lumactl config --tmux-mode statusline
lumactl uninstall
lumactl plugins
lumactl palettes
```

Set `LUMA_NO_LIVE=1` to skip live side effects such as `tmux source-file` during tests or dry runs. Set `LUMA_TMUX_BIN` if tmux is installed outside `PATH`, `/opt/homebrew/bin`, `/usr/local/bin`, or `/usr/bin`.

`lumactl watch` is a long-running process intended to be managed by launchd. It uses native macOS `NSDistributedNotificationCenter` `AppleInterfaceThemeChangedNotification` events for realtime appearance changes, with a conservative fallback poll. Tune the fallback with `LUMA_WATCH_POLL_MS` if needed.

Uninstall unloads the watcher and removes Luma-managed files/blocks while keeping `~/.config/luma/config`:

```bash
lumactl uninstall
```

Watcher status:

```bash
launchctl print gui/$(id -u)/dev.luma.lumactl
```

## Architecture

Workspace packages:

```text
crates/luma-core        # config, palettes, traits/interfaces
crates/luma-os-macos    # macOS appearance backend + launchd install
crates/luma-terminals   # terminal plugins, currently Ghostty and tmux
crates/luma-editors     # terminal editor plugins, currently Nvim
crates/luma-tui         # TUI plugins, currently K9s
crates/luma-harnesses   # agentic harness plugins, currently Pi
crates/lumactl          # CLI orchestration and plugin selection
```

The key interfaces are in `luma-core`:

- `AppearanceBackend` for OS read/set/watch capabilities
- `Platform` for OS-specific app config/cache path resolution
- `LumaPlugin` for app adapters
- marker traits: `Terminal`, `TerminalEditor`, `TerminalUi`, `AgenticHarness`

Plugins are OS-agnostic by default: they render app config and ask `ctx.platform` for paths. Linux and Windows should be added as new `luma-os-*` backend crates that implement `AppearanceBackend + Platform`, rather than splitting every plugin per OS.
