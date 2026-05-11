use std::{env, fs, path::Path, path::PathBuf};

use anyhow::{Result, anyhow};

pub const DEFAULT_LIGHT: &str = "dawnfox";
pub const DEFAULT_DARK: &str = "carbonfox";
pub const THEME_NAME: &str = "luma";
pub const LAUNCH_LABEL: &str = "dev.luma.lumactl";
pub const DEFAULT_PLUGINS: &[&str] = &["nvim", "ghostty", "tmux", "k9s", "pi"];

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OsKind {
    MacOs,
    Linux,
    Windows,
    Unknown,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AppId {
    Luma,
    Nvim,
    Ghostty,
    Tmux,
    K9s,
    Pi,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Mode {
    Light,
    Dark,
}

impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum TmuxMode {
    /// Generic-safe mode: export Luma palette variables without owning the tmux
    /// statusline or window formats.
    #[default]
    Palette,
    /// Opinionated full statusline owned by Luma.
    Statusline,
    /// Do not manage tmux, even if the tmux plugin is enabled.
    Off,
}

impl TmuxMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Palette => "palette",
            Self::Statusline => "statusline",
            Self::Off => "off",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThemeConfig {
    /// Light-mode colorscheme name. This is the canonical theme key used by
    /// Nvim and by built-in Luma palettes for generated targets.
    pub light: String,
    /// Dark-mode colorscheme name. This is the canonical theme key used by
    /// Nvim and by built-in Luma palettes for generated targets.
    pub dark: String,
    /// Optional terminal theme display name override for Ghostty light mode.
    pub light_ghostty: Option<String>,
    /// Optional terminal theme display name override for Ghostty dark mode.
    pub dark_ghostty: Option<String>,
    /// Built-in plugin names to run, e.g. nvim,ghostty,tmux,k9s,pi.
    pub plugins: Vec<String>,
    /// Tmux integration depth. Palette is generic-safe; statusline is
    /// opinionated and owns tmux statusline options.
    pub tmux_mode: TmuxMode,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            light: DEFAULT_LIGHT.to_string(),
            dark: DEFAULT_DARK.to_string(),
            light_ghostty: None,
            dark_ghostty: None,
            plugins: default_plugins(),
            tmux_mode: TmuxMode::default(),
        }
    }
}

impl ThemeConfig {
    pub fn theme_for_mode(&self, mode: Mode) -> &str {
        match mode {
            Mode::Light => &self.light,
            Mode::Dark => &self.dark,
        }
    }

    pub fn ghostty_theme_for_mode(&self, mode: Mode) -> String {
        match mode {
            Mode::Light => self
                .light_ghostty
                .clone()
                .unwrap_or_else(|| terminal_theme_name(&self.light)),
            Mode::Dark => self
                .dark_ghostty
                .clone()
                .unwrap_or_else(|| terminal_theme_name(&self.dark)),
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Palette {
    pub key: &'static str,
    pub name: &'static str,
    pub light: bool,
    pub bg0: &'static str,
    pub bg1: &'static str,
    pub bg2: &'static str,
    pub bg3: &'static str,
    pub bg4: &'static str,
    pub fg0: &'static str,
    pub fg1: &'static str,
    pub fg2: &'static str,
    pub fg3: &'static str,
    pub sel0: &'static str,
    pub sel1: &'static str,
    pub comment: &'static str,
    pub black: &'static str,
    pub red: &'static str,
    pub green: &'static str,
    pub yellow: &'static str,
    pub blue: &'static str,
    pub magenta: &'static str,
    pub cyan: &'static str,
    pub white: &'static str,
    pub orange: &'static str,
    pub pink: &'static str,
}

/// Built-in color scheme definitions.
///
/// Config chooses the light/dark scheme by key (`LUMA_LIGHT`, `LUMA_DARK`).
/// Plugins that need concrete colors (K9s, Pi) look up those keys here. Plugins
/// that natively know the scheme name (Nvim/Ghostty) receive the key/name from
/// config instead of hard-coding colors.
pub const PALETTES: &[Palette] = &[
    Palette {
        key: "carbonfox",
        name: "Carbonfox",
        light: false,
        bg0: "#0c0c0c",
        bg1: "#161616",
        bg2: "#252525",
        bg3: "#353535",
        bg4: "#535353",
        fg0: "#f9fbff",
        fg1: "#f2f4f8",
        fg2: "#b6b8bb",
        fg3: "#7b7c7e",
        sel0: "#2a2a2a",
        sel1: "#525253",
        comment: "#6e6f70",
        black: "#282828",
        red: "#ee5396",
        green: "#25be6a",
        yellow: "#08bdba",
        blue: "#78a9ff",
        magenta: "#be95ff",
        cyan: "#33b1ff",
        white: "#dfdfe0",
        orange: "#3ddbd9",
        pink: "#ff7eb6",
    },
    Palette {
        key: "dawnfox",
        name: "Dawnfox",
        light: true,
        bg0: "#ebe5df",
        bg1: "#faf4ed",
        bg2: "#ebe0df",
        bg3: "#ebdfe4",
        bg4: "#bdbfc9",
        fg0: "#4c4769",
        fg1: "#575279",
        fg2: "#625c87",
        fg3: "#a8a3b3",
        sel0: "#d0d8d8",
        sel1: "#b8cece",
        comment: "#9893a5",
        black: "#575279",
        red: "#b4637a",
        green: "#618774",
        yellow: "#ea9d34",
        blue: "#286983",
        magenta: "#907aa9",
        cyan: "#56949f",
        white: "#e5e9f0",
        orange: "#d7827e",
        pink: "#d685af",
    },
    Palette {
        key: "nightfox",
        name: "Nightfox",
        light: false,
        bg0: "#131a24",
        bg1: "#192330",
        bg2: "#212e3f",
        bg3: "#29394f",
        bg4: "#39506d",
        fg0: "#d6d6d7",
        fg1: "#cdcecf",
        fg2: "#aeafb0",
        fg3: "#71839b",
        sel0: "#2b3b51",
        sel1: "#3c5372",
        comment: "#738091",
        black: "#393b44",
        red: "#c94f6d",
        green: "#81b29a",
        yellow: "#dbc074",
        blue: "#719cd6",
        magenta: "#9d79d6",
        cyan: "#63cdcf",
        white: "#dfdfe0",
        orange: "#f4a261",
        pink: "#d67ad2",
    },
    Palette {
        key: "dayfox",
        name: "Dayfox",
        light: true,
        bg0: "#e4dcd4",
        bg1: "#f6f2ee",
        bg2: "#dbd1dd",
        bg3: "#d3c7bb",
        bg4: "#aab0ad",
        fg0: "#302b5d",
        fg1: "#3d2b5a",
        fg2: "#643f61",
        fg3: "#824d5b",
        sel0: "#e7d2be",
        sel1: "#a4c1c2",
        comment: "#837a72",
        black: "#352c24",
        red: "#a5222f",
        green: "#396847",
        yellow: "#ac5402",
        blue: "#2848a9",
        magenta: "#6e33ce",
        cyan: "#287980",
        white: "#f2e9e1",
        orange: "#955f61",
        pink: "#a440b5",
    },
    Palette {
        key: "duskfox",
        name: "Duskfox",
        light: false,
        bg0: "#191726",
        bg1: "#232136",
        bg2: "#2d2a45",
        bg3: "#373354",
        bg4: "#4b4673",
        fg0: "#eae8ff",
        fg1: "#e0def4",
        fg2: "#cdcbe0",
        fg3: "#6e6a86",
        sel0: "#433c59",
        sel1: "#63577d",
        comment: "#817c9c",
        black: "#393552",
        red: "#eb6f92",
        green: "#a3be8c",
        yellow: "#f6c177",
        blue: "#569fba",
        magenta: "#c4a7e7",
        cyan: "#9ccfd8",
        white: "#e0def4",
        orange: "#ea9a97",
        pink: "#eb98c3",
    },
    Palette {
        key: "nordfox",
        name: "Nordfox",
        light: false,
        bg0: "#232831",
        bg1: "#2e3440",
        bg2: "#39404f",
        bg3: "#444c5e",
        bg4: "#5a657d",
        fg0: "#c7cdd9",
        fg1: "#cdcecf",
        fg2: "#abb1bb",
        fg3: "#7e8188",
        sel0: "#3e4a5b",
        sel1: "#4f6074",
        comment: "#60728a",
        black: "#3b4252",
        red: "#bf616a",
        green: "#a3be8c",
        yellow: "#ebcb8b",
        blue: "#81a1c1",
        magenta: "#b48ead",
        cyan: "#88c0d0",
        white: "#e5e9f0",
        orange: "#c9826b",
        pink: "#bf88bc",
    },
    Palette {
        key: "terafox",
        name: "Terafox",
        light: false,
        bg0: "#0f1c1e",
        bg1: "#152528",
        bg2: "#1d3337",
        bg3: "#254147",
        bg4: "#2d4f56",
        fg0: "#eaeeee",
        fg1: "#e6eaea",
        fg2: "#cbd9d8",
        fg3: "#587b7b",
        sel0: "#293e40",
        sel1: "#425e5e",
        comment: "#6d7f8b",
        black: "#2f3239",
        red: "#e85c51",
        green: "#7aa4a1",
        yellow: "#fda47f",
        blue: "#5a93aa",
        magenta: "#ad5c7c",
        cyan: "#a1cdd8",
        white: "#ebebeb",
        orange: "#ff8349",
        pink: "#cb7985",
    },
];

pub struct SyncContext<'a> {
    pub platform: &'a dyn Platform,
    pub mode: Mode,
    pub theme: String,
    pub config: ThemeConfig,
    pub palette: Palette,
}

impl<'a> SyncContext<'a> {
    pub fn new(mode: Mode, config: ThemeConfig, platform: &'a dyn Platform) -> Result<Self> {
        let theme = config.theme_for_mode(mode).to_string();
        let palette = palette_for(mode, &theme);
        Ok(Self {
            platform,
            mode,
            theme,
            config,
            palette,
        })
    }
}

pub trait LumaPlugin {
    fn name(&self) -> &'static str;
    fn sync(&self, ctx: &SyncContext<'_>) -> Result<()>;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct AppearanceCapabilities {
    pub can_read: bool,
    pub can_set: bool,
    pub can_watch: bool,
}

pub trait Platform {
    fn os_kind(&self) -> OsKind;
    fn home_dir(&self) -> Result<PathBuf>;
    fn app_config_files(&self, app: AppId, relative: &Path) -> Result<Vec<PathBuf>>;
    fn app_cache_file(&self, app: AppId, relative: &Path) -> Result<PathBuf>;
}

pub trait AppearanceBackend: Platform {
    fn name(&self) -> &'static str;
    fn capabilities(&self) -> AppearanceCapabilities;
    fn current_mode(&self) -> Result<Mode>;
    fn set_mode(&self, mode: DesiredMode) -> Result<()>;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DesiredMode {
    Toggle,
    Dark,
    Light,
}

pub trait Terminal: LumaPlugin {}
pub trait TerminalEditor: LumaPlugin {}
pub trait TerminalUi: LumaPlugin {}
pub trait AgenticHarness: LumaPlugin {}

pub fn palette_for(mode: Mode, requested_theme: &str) -> Palette {
    let normalized = normalize_theme_name(requested_theme);
    if let Some(palette) = PALETTES.iter().find(|palette| palette.key == normalized) {
        return *palette;
    }

    let fallback = match mode {
        Mode::Light => DEFAULT_LIGHT,
        Mode::Dark => DEFAULT_DARK,
    };
    eprintln!("luma: palette for {normalized:?} is not built in; using {fallback}");
    *PALETTES
        .iter()
        .find(|palette| palette.key == fallback)
        .expect("default palette exists")
}

pub fn read_theme_config() -> Result<ThemeConfig> {
    let path = config_file()?;
    let mut cfg = ThemeConfig::default();
    if !path.exists() {
        return Ok(cfg);
    }

    let text = fs::read_to_string(&path)?;
    for raw_line in text.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = parse_config_value(value.trim());
        match key.trim() {
            "LUMA_LIGHT" => cfg.light = normalize_theme_name(&value),
            "LUMA_DARK" => cfg.dark = normalize_theme_name(&value),
            "LUMA_LIGHT_GHOSTTY" => cfg.light_ghostty = non_empty(value),
            "LUMA_DARK_GHOSTTY" => cfg.dark_ghostty = non_empty(value),
            "LUMA_PLUGINS" => cfg.plugins = parse_plugin_list(&value),
            "LUMA_TMUX_MODE" => {
                if let Some(mode) = parse_tmux_mode(&value) {
                    cfg.tmux_mode = mode;
                } else {
                    eprintln!("luma: ignoring unknown LUMA_TMUX_MODE={value:?}");
                }
            }
            _ => {}
        }
    }
    Ok(cfg)
}

pub fn write_theme_config(cfg: &ThemeConfig) -> Result<()> {
    fs::create_dir_all(config_dir()?)?;
    let mut text = String::new();
    text.push_str("# Managed by luma/lumactl. Safe to edit.\n");
    text.push_str("# Color scheme keys select entries from luma-core::PALETTES.\n");
    text.push_str("# Plugins are built-in adapters to run: nvim,ghostty,tmux,k9s,pi.\n");
    text.push_str("# Tmux modes: palette (generic-safe vars), statusline (Luma-owned bar), off.\n");
    text.push_str(&format!("LUMA_LIGHT={}\n", shell_quote(&cfg.light)));
    text.push_str(&format!("LUMA_DARK={}\n", shell_quote(&cfg.dark)));
    if let Some(light) = &cfg.light_ghostty {
        text.push_str(&format!("LUMA_LIGHT_GHOSTTY={}\n", shell_quote(light)));
    }
    if let Some(dark) = &cfg.dark_ghostty {
        text.push_str(&format!("LUMA_DARK_GHOSTTY={}\n", shell_quote(dark)));
    }
    text.push_str(&format!(
        "LUMA_PLUGINS={}\n",
        shell_quote(&cfg.plugins.join(","))
    ));
    text.push_str(&format!("LUMA_TMUX_MODE={}\n", cfg.tmux_mode.as_str()));
    write_if_changed(&config_file()?, &text)
}

pub fn print_theme_config(cfg: &ThemeConfig) {
    println!("LUMA_LIGHT={}", cfg.light);
    println!("LUMA_DARK={}", cfg.dark);
    if let Some(light) = &cfg.light_ghostty {
        println!("LUMA_LIGHT_GHOSTTY={light}");
    }
    if let Some(dark) = &cfg.dark_ghostty {
        println!("LUMA_DARK_GHOSTTY={dark}");
    }
    println!("LUMA_PLUGINS={}", cfg.plugins.join(","));
    println!("LUMA_TMUX_MODE={}", cfg.tmux_mode.as_str());
}

pub fn parse_tmux_mode(value: &str) -> Option<TmuxMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "palette" | "vars" | "variables" => Some(TmuxMode::Palette),
        "statusline" | "status" | "bar" | "full" => Some(TmuxMode::Statusline),
        "off" | "none" | "disabled" | "disable" => Some(TmuxMode::Off),
        _ => None,
    }
}

pub fn normalize_theme_name(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

pub fn terminal_theme_name(value: &str) -> String {
    value
        .split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

pub fn parse_config_value(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
        return value[1..value.len() - 1]
            .replace("\\\"", "\"")
            .replace("\\\\", "\\");
    }
    if value.len() >= 2 && value.starts_with('\'') && value.ends_with('\'') {
        return value[1..value.len() - 1].replace("'\\''", "'");
    }
    value.to_string()
}

pub fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '/' | ','))
    {
        return value.to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

pub fn default_plugins() -> Vec<String> {
    DEFAULT_PLUGINS
        .iter()
        .map(|plugin| (*plugin).to_string())
        .collect()
}

pub fn parse_plugin_list(value: &str) -> Vec<String> {
    let plugins: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|plugin| !plugin.is_empty())
        .map(|plugin| plugin.to_ascii_lowercase())
        .collect();
    if plugins.is_empty() {
        default_plugins()
    } else {
        plugins
    }
}

pub fn write_if_changed(path: &Path, content: &str) -> Result<()> {
    if let Ok(existing) = fs::read_to_string(path)
        && existing == content
    {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

pub fn canonical_or_self(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

pub fn home_dir() -> Result<PathBuf> {
    env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("HOME is not set"))
}

pub fn config_dir() -> Result<PathBuf> {
    Ok(env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or(home_dir()?.join(".config"))
        .join("luma"))
}

pub fn config_file() -> Result<PathBuf> {
    Ok(config_dir()?.join("config"))
}

pub fn cache_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".cache/luma"))
}

pub fn local_bin_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join(".local/bin"))
}

pub fn launch_agents_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join("Library/LaunchAgents"))
}

pub fn supported_palette_names() -> String {
    PALETTES
        .iter()
        .map(|palette| palette.key)
        .collect::<Vec<_>>()
        .join(", ")
}
