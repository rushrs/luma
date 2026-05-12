// SPDX-License-Identifier: MIT
use std::{env, fs, path::Path, path::PathBuf, process::Command};

use anyhow::{Result, bail};
use luma_core::{
    AppId, LumaPlugin, Palette, Platform, SyncContext, THEME_NAME, Terminal, TmuxMode,
    primary_app_config_file, read_optional_text, write_if_changed,
};

#[derive(Debug, Default)]
pub struct Tmux;

impl LumaPlugin for Tmux {
    fn name(&self) -> &'static str {
        "tmux"
    }

    fn sync(&self, ctx: &SyncContext) -> Result<()> {
        write_tmux_theme(ctx)?;
        match ctx.config.tmux_mode {
            TmuxMode::Off => remove_tmux_conf_source_theme(ctx.platform)?,
            TmuxMode::Palette | TmuxMode::Statusline => {
                ensure_tmux_conf_sources_theme(ctx.platform)?
            }
        }
        apply_tmux_theme_live(ctx.platform)?;
        Ok(())
    }
}

impl Terminal for Tmux {}

pub fn tmux_theme_file(platform: &dyn Platform) -> Result<PathBuf> {
    primary_app_config_file(
        platform,
        AppId::Tmux,
        Path::new(&format!("{THEME_NAME}.tmux.conf")),
    )
}

pub fn tmux_config_file(platform: &dyn Platform) -> Result<PathBuf> {
    primary_app_config_file(platform, AppId::Tmux, Path::new("tmux.conf"))
}

fn write_tmux_theme(ctx: &SyncContext) -> Result<()> {
    let path = tmux_theme_file(ctx.platform)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = render_tmux_theme(ctx)?;

    write_if_changed(&path, &text)
}

fn ensure_tmux_conf_sources_theme(platform: &dyn Platform) -> Result<()> {
    let config = tmux_config_file(platform)?;
    if let Some(parent) = config.parent() {
        fs::create_dir_all(parent)?;
    }
    let theme = tmux_theme_file(platform)?;
    let source_line = format!("source-file {}", shell_quote_path(&theme));
    let marker_start = "# >>> luma";
    let marker_end = "# <<< luma";
    let block = format!("{marker_start}\n{source_line}\n{marker_end}\n");

    let text = read_optional_text(&config)?.unwrap_or_default();
    match (text.contains(marker_start), text.contains(marker_end)) {
        (true, true) => {
            let next = replace_marked_block(&text, marker_start, marker_end, &block);
            return write_if_changed(&config, &next);
        }
        (false, false) => {}
        _ => bail!(
            "refusing to edit {} because its Luma tmux marker block is incomplete",
            config.display()
        ),
    }
    if text.lines().any(|line| line.trim() == source_line) {
        return Ok(());
    }

    let mut next = text;
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next.push('\n');
    next.push_str(&block);
    write_if_changed(&config, &next)
}

fn remove_tmux_conf_source_theme(platform: &dyn Platform) -> Result<()> {
    let config = tmux_config_file(platform)?;
    let Some(text) = read_optional_text(&config)? else {
        return Ok(());
    };
    let marker_start = "# >>> luma";
    let marker_end = "# <<< luma";

    match (text.contains(marker_start), text.contains(marker_end)) {
        (true, true) => {
            let next = remove_marked_block(&text, marker_start, marker_end);
            write_if_changed(&config, &next)
        }
        (false, false) => Ok(()),
        _ => bail!(
            "refusing to edit {} because its Luma tmux marker block is incomplete",
            config.display()
        ),
    }
}

fn apply_tmux_theme_live(platform: &dyn Platform) -> Result<()> {
    let theme = tmux_theme_file(platform)?;
    source_tmux_file_live(&theme);
    Ok(())
}

fn source_tmux_file_live(path: &Path) {
    if env::var_os("LUMA_NO_LIVE").is_some() {
        return;
    }
    let Some(tmux) = tmux_bin() else {
        return;
    };
    let _ = Command::new(&tmux).arg("source-file").arg(path).status();
    refresh_tmux_clients(&tmux);
}

fn refresh_tmux_clients(tmux: &Path) {
    let Ok(output) = Command::new(tmux)
        .args(["list-clients", "-F", "#{client_name}"])
        .output()
    else {
        let _ = Command::new(tmux).args(["refresh-client", "-S"]).status();
        return;
    };

    if !output.status.success() {
        let _ = Command::new(tmux).args(["refresh-client", "-S"]).status();
        return;
    }

    let clients = String::from_utf8_lossy(&output.stdout);
    let mut refreshed = false;
    for client in clients
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
    {
        let _ = Command::new(tmux)
            .args(["refresh-client", "-S", "-t", client])
            .status();
        refreshed = true;
    }
    if !refreshed {
        let _ = Command::new(tmux).args(["refresh-client", "-S"]).status();
    }
}

fn tmux_bin() -> Option<PathBuf> {
    if let Some(path) = env::var_os("LUMA_TMUX_BIN").map(PathBuf::from)
        && path.is_file()
    {
        return Some(path);
    }

    if let Some(path) = find_executable_in_path("tmux") {
        return Some(path);
    }

    [
        "/opt/homebrew/bin/tmux",
        "/usr/local/bin/tmux",
        "/usr/bin/tmux",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|path| path.is_file())
}

fn find_executable_in_path(name: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|path| {
        env::split_paths(&path)
            .map(|dir| dir.join(name))
            .find(|candidate| candidate.is_file())
    })
}

fn render_tmux_theme(ctx: &SyncContext) -> Result<String> {
    match ctx.config.tmux_mode {
        TmuxMode::Palette => Ok(render_tmux_palette_theme(ctx)),
        TmuxMode::Statusline => render_tmux_statusline_theme(ctx),
        TmuxMode::Off => Ok(render_tmux_off_theme(ctx)),
    }
}

fn render_tmux_statusline_theme(ctx: &SyncContext) -> Result<String> {
    let p = &ctx.palette;
    let colors = tmux_colors(p);
    let status_right = format!(
        "#[fg={muted},bg={bar_bg}]#{{b:pane_current_path}} #[fg={session_bg},bg={bar_bg}]#[fg={text_dark},bg={session_bg},bold] %H:%M #[fg={panel_bg},bg={session_bg}]#[fg={fg},bg={panel_bg}] #H ",
        muted = colors.muted,
        bar_bg = colors.bar_bg,
        session_bg = colors.session_bg,
        text_dark = colors.text_dark,
        panel_bg = colors.panel_bg,
        fg = colors.fg,
    );

    let pane_label = "#W";

    let palette_vars = render_tmux_palette_vars(ctx, &colors, TmuxMode::Statusline);

    Ok(format!(
        r##"# Generated by Luma. Do not edit directly.
# Theme: {theme} ({mode})
{palette_vars}
set -g status on
set -g status-position bottom
set -g status-interval 1
set -g status-justify left
set -g status-style "fg={fg},bg={bar_bg}"
set -g status-left-style "bg={bar_bg}"
set -g status-right-style "bg={bar_bg}"
set -g message-style "fg={text_dark},bg={session_bg},bold"
set -g mode-style "fg={text_dark},bg={session_bg},bold"

set -g status-left-length 30
set -g status-left "#[fg={text_dark},bg={session_bg},bold] #S #[fg={session_bg},bg={bar_bg},nobold,noitalics,nounderscore]#[fg={muted},bg={bar_bg}] "

set -g window-status-separator ""
set -g window-status-style "fg={muted},bg={bar_bg}"
set -g window-status-current-style "bold"
set -g window-status-format "#[fg={muted},bg={bar_bg}] #I:{pane_label}#F "
set -g window-status-current-format "#[fg={bar_bg},bg={active_bg},nobold,noitalics,nounderscore]#[fg={text_dark},bg={active_bg},bold] #I:{pane_label}#F #[fg={active_bg},bg={bar_bg},nobold,noitalics,nounderscore]"
set -g window-status-activity-style "fg={active_bg},bg={bar_bg}"
set-option -gq window-status-activity-attr none

set -g status-right-length 220
set -g status-right "{status_right}"

set -g pane-border-style "fg={border}"
set -g pane-active-border-style "fg={session_bg}"
set -g window-style ""
set -g window-active-style ""
refresh-client -S
"##,
        theme = tmux_comment_value(&ctx.theme),
        mode = ctx.mode.as_str(),
        palette_vars = palette_vars,
        fg = colors.fg,
        bar_bg = colors.bar_bg,
        muted = colors.muted,
        session_bg = colors.session_bg,
        active_bg = colors.active_bg,
        text_dark = colors.text_dark,
        border = colors.border,
        pane_label = pane_label,
        status_right = escape_tmux_double_quoted(&status_right),
    ))
}

fn render_tmux_palette_theme(ctx: &SyncContext) -> String {
    let colors = tmux_colors(&ctx.palette);
    let palette_vars = render_tmux_palette_vars(ctx, &colors, TmuxMode::Palette);
    format!(
        "# Generated by Luma. Do not edit directly.\n# Tmux palette mode: no statusline options are changed.\n{palette_vars}"
    )
}

fn render_tmux_off_theme(ctx: &SyncContext) -> String {
    format!(
        r#"# Generated by Luma. Do not edit directly.
# Tmux mode is off; this file unsets options previously owned by Luma statusline mode.
set -gq @luma_theme "{theme}"
set -gq @luma_mode "{appearance_mode}"
set -gq @luma_tmux_mode "off"

set -guq status
set -guq status-position
set -guq status-interval
set -guq status-justify
set -guq status-style
set -guq status-left-style
set -guq status-right-style
set -guq message-style
set -guq mode-style
set -guq status-left-length
set -guq status-left
set -guq window-status-separator
set -guq window-status-style
set -guq window-status-current-style
set -guq window-status-format
set -guq window-status-current-format
set -guq window-status-activity-style
set-option -guq window-status-activity-attr
set -guq status-right-length
set -guq status-right
set -guq pane-border-style
set -guq pane-active-border-style
set -guq window-style
set -guq window-active-style
refresh-client -S
"#,
        theme = escape_tmux_double_quoted(&ctx.theme),
        appearance_mode = ctx.mode.as_str(),
    )
}

fn render_tmux_palette_vars(ctx: &SyncContext, colors: &TmuxColors<'_>, mode: TmuxMode) -> String {
    let p = &ctx.palette;
    format!(
        r#"set -gq @luma_theme "{theme}"
set -gq @luma_mode "{appearance_mode}"
set -gq @luma_tmux_mode "{tmux_mode}"
set -gq @luma_color_bg0 "{bg0}"
set -gq @luma_color_bg1 "{bg1}"
set -gq @luma_color_bg2 "{bg2}"
set -gq @luma_color_bg3 "{bg3}"
set -gq @luma_color_bg4 "{bg4}"
set -gq @luma_color_fg0 "{fg0}"
set -gq @luma_color_fg1 "{fg1}"
set -gq @luma_color_fg2 "{fg2}"
set -gq @luma_color_fg3 "{fg3}"
set -gq @luma_color_sel0 "{sel0}"
set -gq @luma_color_sel1 "{sel1}"
set -gq @luma_color_comment "{comment}"
set -gq @luma_color_black "{black}"
set -gq @luma_color_red "{red}"
set -gq @luma_color_green "{green}"
set -gq @luma_color_yellow "{yellow}"
set -gq @luma_color_blue "{blue}"
set -gq @luma_color_magenta "{magenta}"
set -gq @luma_color_cyan "{cyan}"
set -gq @luma_color_white "{white}"
set -gq @luma_color_orange "{orange}"
set -gq @luma_color_pink "{pink}"
set -gq @luma_tmux_bar_bg "{bar_bg}"
set -gq @luma_tmux_panel_bg "{panel_bg}"
set -gq @luma_tmux_fg "{fg}"
set -gq @luma_tmux_muted "{muted}"
set -gq @luma_tmux_session_bg "{session_bg}"
set -gq @luma_tmux_active_bg "{active_bg}"
set -gq @luma_tmux_text_dark "{text_dark}"
set -gq @luma_tmux_border "{border}"
set -gq @luma_tmux_waiting "{waiting}"
"#,
        theme = escape_tmux_double_quoted(&ctx.theme),
        appearance_mode = ctx.mode.as_str(),
        tmux_mode = mode.as_str(),
        bg0 = p.bg0,
        bg1 = p.bg1,
        bg2 = p.bg2,
        bg3 = p.bg3,
        bg4 = p.bg4,
        fg0 = p.fg0,
        fg1 = p.fg1,
        fg2 = p.fg2,
        fg3 = p.fg3,
        sel0 = p.sel0,
        sel1 = p.sel1,
        comment = p.comment,
        black = p.black,
        red = p.red,
        green = p.green,
        yellow = p.yellow,
        blue = p.blue,
        magenta = p.magenta,
        cyan = p.cyan,
        white = p.white,
        orange = p.orange,
        pink = p.pink,
        bar_bg = colors.bar_bg,
        panel_bg = colors.panel_bg,
        fg = colors.fg,
        muted = colors.muted,
        session_bg = colors.session_bg,
        active_bg = colors.active_bg,
        text_dark = colors.text_dark,
        border = colors.border,
        waiting = colors.waiting,
    )
}

struct TmuxColors<'a> {
    bar_bg: &'a str,
    panel_bg: &'a str,
    fg: &'a str,
    muted: &'a str,
    session_bg: &'a str,
    active_bg: &'a str,
    text_dark: &'a str,
    border: &'a str,
    waiting: &'a str,
}

fn tmux_colors(p: &Palette) -> TmuxColors<'_> {
    if p.light {
        TmuxColors {
            bar_bg: &p.white,
            panel_bg: &p.bg1,
            fg: &p.fg1,
            muted: &p.fg2,
            session_bg: &p.blue,
            active_bg: &p.yellow,
            text_dark: &p.bg1,
            border: &p.bg4,
            waiting: &p.red,
        }
    } else {
        TmuxColors {
            bar_bg: &p.bg1,
            panel_bg: &p.bg2,
            fg: &p.fg1,
            muted: &p.fg2,
            session_bg: &p.blue,
            active_bg: &p.magenta,
            text_dark: &p.bg1,
            border: &p.bg3,
            waiting: &p.red,
        }
    }
}

fn replace_marked_block(text: &str, start: &str, end: &str, replacement: &str) -> String {
    let Some(start_idx) = text.find(start) else {
        return text.to_string();
    };
    let Some(relative_end_idx) = text[start_idx..].find(end) else {
        return text.to_string();
    };
    let end_idx = start_idx + relative_end_idx + end.len();
    let mut next = String::new();
    next.push_str(&text[..start_idx]);
    next.push_str(replacement);
    if let Some(suffix) = text.get(end_idx..) {
        next.push_str(suffix.trim_start_matches('\n'));
    }
    if !next.ends_with('\n') {
        next.push('\n');
    }
    next
}

fn remove_marked_block(text: &str, start: &str, end: &str) -> String {
    replace_marked_block(text, start, end, "")
}

fn shell_quote_path(path: &Path) -> String {
    shell_quote(&path.to_string_lossy())
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn escape_tmux_double_quoted(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            '\\' => "\\\\".to_string(),
            '"' => "\\\"".to_string(),
            '\n' | '\r' => " ".to_string(),
            _ => ch.to_string(),
        })
        .collect()
}

fn tmux_comment_value(value: &str) -> String {
    value.replace(['\n', '\r'], " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use luma_core::{Mode, OsKind, ThemeConfig};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Result<Self> {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock is before UNIX_EPOCH")
                .as_nanos();
            let path = std::env::temp_dir()
                .join(format!("luma-tmux-{name}-{}-{unique}", std::process::id()));
            fs::create_dir_all(&path)?;
            Ok(Self(path))
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    struct FakePlatform {
        root: PathBuf,
        empty: bool,
    }

    impl Platform for FakePlatform {
        fn os_kind(&self) -> OsKind {
            OsKind::Unknown
        }

        fn home_dir(&self) -> Result<PathBuf> {
            Ok(self.root.clone())
        }

        fn app_config_files(&self, app: AppId, relative: &Path) -> Result<Vec<PathBuf>> {
            if self.empty {
                return Ok(Vec::new());
            }
            Ok(vec![self.root.join(format!("{app:?}")).join(relative)])
        }

        fn app_cache_file(&self, app: AppId, relative: &Path) -> Result<PathBuf> {
            Ok(self
                .root
                .join("cache")
                .join(format!("{app:?}"))
                .join(relative))
        }
    }

    #[test]
    fn tmux_path_helpers_return_errors_for_empty_platform_paths() {
        let platform = FakePlatform {
            root: PathBuf::new(),
            empty: true,
        };

        assert!(tmux_theme_file(&platform).is_err());
        assert!(tmux_config_file(&platform).is_err());
    }

    #[test]
    fn off_mode_removes_owned_tmux_source_block() -> Result<()> {
        let temp = TempDir::new("remove-block")?;
        let platform = FakePlatform {
            root: temp.0.clone(),
            empty: false,
        };
        let config = tmux_config_file(&platform)?;
        fs::create_dir_all(config.parent().expect("tmux config has parent"))?;
        fs::write(
            &config,
            "set -g mouse on\n# >>> luma\nsource-file '~/.tmux/luma.tmux.conf'\n# <<< luma\nset -g history-limit 10000\n",
        )?;

        remove_tmux_conf_source_theme(&platform)?;

        let next = fs::read_to_string(config)?;
        assert!(next.contains("set -g mouse on"));
        assert!(next.contains("set -g history-limit 10000"));
        assert!(!next.contains("# >>> luma"));
        assert!(!next.contains("source-file '~/.tmux/luma.tmux.conf'"));
        Ok(())
    }

    #[test]
    fn incomplete_tmux_marker_block_fails_closed() -> Result<()> {
        let temp = TempDir::new("incomplete-block")?;
        let platform = FakePlatform {
            root: temp.0.clone(),
            empty: false,
        };
        let config = tmux_config_file(&platform)?;
        fs::create_dir_all(config.parent().expect("tmux config has parent"))?;
        fs::write(&config, "set -g mouse on\n# >>> luma\n")?;

        let err = ensure_tmux_conf_sources_theme(&platform)
            .expect_err("incomplete marker should not be edited");

        assert!(err.to_string().contains("marker block is incomplete"));
        assert_eq!(fs::read_to_string(config)?, "set -g mouse on\n# >>> luma\n");
        Ok(())
    }

    #[test]
    fn off_theme_unsets_statusline_options_and_escapes_theme() -> Result<()> {
        let temp = TempDir::new("off-render")?;
        let platform = FakePlatform {
            root: temp.0.clone(),
            empty: false,
        };
        let cfg = ThemeConfig {
            dark: "quote\"backslash\\theme".to_string(),
            tmux_mode: TmuxMode::Off,
            ..ThemeConfig::default()
        };
        let ctx = SyncContext::new(Mode::Dark, cfg, &platform)?;

        let text = render_tmux_off_theme(&ctx);

        assert!(text.contains("set -guq status-left"));
        assert!(text.contains("set -gq @luma_theme \"quote\\\"backslash\\\\theme\""));
        Ok(())
    }
}
