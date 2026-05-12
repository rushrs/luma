// SPDX-License-Identifier: MIT
use std::{
    collections::BTreeSet,
    env, fs,
    io::ErrorKind,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;

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
    /// Optional directory containing custom JSON palette definitions.
    /// Defaults to `$XDG_CONFIG_HOME/luma/themes` or `~/.config/luma/themes`.
    pub theme_dir: Option<PathBuf>,
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
            theme_dir: None,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Palette {
    pub key: String,
    pub name: String,
    pub light: bool,
    pub bg0: String,
    pub bg1: String,
    pub bg2: String,
    pub bg3: String,
    pub bg4: String,
    pub fg0: String,
    pub fg1: String,
    pub fg2: String,
    pub fg3: String,
    pub sel0: String,
    pub sel1: String,
    pub comment: String,
    pub black: String,
    pub red: String,
    pub green: String,
    pub yellow: String,
    pub blue: String,
    pub magenta: String,
    pub cyan: String,
    pub white: String,
    pub orange: String,
    pub pink: String,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct BuiltinPalette {
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

impl From<&BuiltinPalette> for Palette {
    fn from(value: &BuiltinPalette) -> Self {
        Self {
            key: value.key.to_string(),
            name: value.name.to_string(),
            light: value.light,
            bg0: value.bg0.to_string(),
            bg1: value.bg1.to_string(),
            bg2: value.bg2.to_string(),
            bg3: value.bg3.to_string(),
            bg4: value.bg4.to_string(),
            fg0: value.fg0.to_string(),
            fg1: value.fg1.to_string(),
            fg2: value.fg2.to_string(),
            fg3: value.fg3.to_string(),
            sel0: value.sel0.to_string(),
            sel1: value.sel1.to_string(),
            comment: value.comment.to_string(),
            black: value.black.to_string(),
            red: value.red.to_string(),
            green: value.green.to_string(),
            yellow: value.yellow.to_string(),
            blue: value.blue.to_string(),
            magenta: value.magenta.to_string(),
            cyan: value.cyan.to_string(),
            white: value.white.to_string(),
            orange: value.orange.to_string(),
            pink: value.pink.to_string(),
        }
    }
}

/// Built-in color scheme definitions.
///
/// Config chooses the light/dark scheme by key (`LUMA_LIGHT`, `LUMA_DARK`).
/// Plugins that need concrete colors (K9s, Pi) look up those keys here. Plugins
/// that natively know the scheme name (Nvim/Ghostty) receive the key/name from
/// config instead of hard-coding colors.
pub const PALETTES: &[BuiltinPalette] = &[
    BuiltinPalette {
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
    BuiltinPalette {
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
    BuiltinPalette {
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
    BuiltinPalette {
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
    BuiltinPalette {
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
    BuiltinPalette {
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
    BuiltinPalette {
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
        let palette = palette_for_config(mode, &theme, &config)?;
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

pub fn primary_app_config_file(
    platform: &dyn Platform,
    app: AppId,
    relative: &Path,
) -> Result<PathBuf> {
    let relative = validate_relative_path(relative)?;
    platform
        .app_config_files(app, relative)?
        .into_iter()
        .next()
        .ok_or_else(|| {
            anyhow!(
                "platform returned no config file for {app:?}/{}",
                relative.display()
            )
        })
}

pub fn validate_relative_path(relative: &Path) -> Result<&Path> {
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::Prefix(_) | Component::RootDir
            )
        })
    {
        return Err(anyhow!(
            "path must be relative and must not contain parent components: {}",
            relative.display()
        ));
    }
    Ok(relative)
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

pub const REQUIRED_COLOR_FIELDS: &[&str] = &[
    "bg0", "bg1", "bg2", "bg3", "bg4", "fg0", "fg1", "fg2", "fg3", "sel0", "sel1", "comment",
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white", "orange", "pink",
];

pub fn palette_for(mode: Mode, requested_theme: &str) -> Palette {
    let normalized = normalize_theme_name(requested_theme);
    if let Some(palette) = builtin_palette(&normalized) {
        return palette;
    }

    fallback_palette(mode, &normalized)
}

pub fn palette_for_config(mode: Mode, requested_theme: &str, cfg: &ThemeConfig) -> Result<Palette> {
    let normalized = normalize_theme_name(requested_theme);

    if let Some(path) = custom_palette_file(cfg, &normalized)? {
        match path.try_exists() {
            Ok(true) => {
                return read_custom_palette_file(&path, &normalized, Some(mode)).with_context(
                    || format!("failed to load custom Luma palette {}", path.display()),
                );
            }
            Ok(false) => {}
            Err(err) => {
                return Err(err).with_context(|| {
                    format!("failed to inspect custom Luma palette {}", path.display())
                });
            }
        }
    }

    if let Some(palette) = builtin_palette(&normalized) {
        return Ok(palette);
    }

    Ok(fallback_palette(mode, &normalized))
}

fn builtin_palette(normalized: &str) -> Option<Palette> {
    PALETTES
        .iter()
        .find(|palette| palette.key == normalized)
        .map(Palette::from)
}

fn fallback_palette(mode: Mode, normalized: &str) -> Palette {
    let fallback = match mode {
        Mode::Light => DEFAULT_LIGHT,
        Mode::Dark => DEFAULT_DARK,
    };
    eprintln!("luma: palette for {normalized:?} is not built in; using {fallback}");
    builtin_palette(fallback).expect("default palette exists")
}

pub fn available_palette_names(cfg: &ThemeConfig) -> Result<String> {
    let mut names: BTreeSet<String> = PALETTES
        .iter()
        .map(|palette| palette.key.to_string())
        .collect();

    for palette in custom_palettes(cfg)? {
        names.insert(palette.key);
    }

    Ok(names.into_iter().collect::<Vec<_>>().join(", "))
}

pub fn custom_palettes(cfg: &ThemeConfig) -> Result<Vec<Palette>> {
    let theme_dir = theme_dir_for_config(cfg)?;
    if !theme_dir.exists() {
        return Ok(Vec::new());
    }

    let mut palettes = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in fs::read_dir(&theme_dir)
        .with_context(|| format!("failed to read Luma theme dir {}", theme_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let Some(key) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let normalized = normalize_theme_name(key);
        if !is_theme_key_safe(&normalized) {
            bail!(
                "custom Luma palette filename must be a simple theme key: {}",
                path.display()
            );
        }
        if !seen.insert(normalized.clone()) {
            bail!("duplicate custom Luma palette key {normalized:?}");
        }
        palettes.push(
            read_custom_palette_file(&path, &normalized, None).with_context(|| {
                format!("failed to load custom Luma palette {}", path.display())
            })?,
        );
    }
    palettes.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(palettes)
}

pub fn validate_theme_file(path: &Path) -> Result<Palette> {
    let key = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .map(normalize_theme_name)
        .ok_or_else(|| anyhow!("theme path must have a UTF-8 file stem: {}", path.display()))?;
    read_custom_palette_file(path, &key, None)
}

fn read_custom_palette_file(path: &Path, key: &str, mode_hint: Option<Mode>) -> Result<Palette> {
    if !is_theme_key_safe(key) {
        bail!("custom Luma palette key must be simple ASCII: {key:?}");
    }

    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read custom palette {}", path.display()))?;
    let value: Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse custom palette JSON {}", path.display()))?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("custom palette {} must be a JSON object", path.display()))?;

    if let Some(declared_key) = optional_string(object, "key")? {
        let declared_key = normalize_theme_name(&declared_key);
        if declared_key != key {
            bail!(
                "custom palette {} declares key {:?}, but filename key is {:?}",
                path.display(),
                declared_key,
                key
            );
        }
    }

    let colors = match object.get("colors") {
        Some(Value::Object(colors)) => Some(colors),
        Some(_) => bail!("custom palette field \"colors\" must be an object"),
        None => None,
    };
    let light = optional_bool(object, "light")?.unwrap_or(matches!(mode_hint, Some(Mode::Light)));
    let name = optional_string(object, "name")?.unwrap_or_else(|| terminal_theme_name(key));

    Ok(Palette {
        key: key.to_string(),
        name,
        light,
        bg0: required_color(object, colors, "bg0")?,
        bg1: required_color(object, colors, "bg1")?,
        bg2: required_color(object, colors, "bg2")?,
        bg3: required_color(object, colors, "bg3")?,
        bg4: required_color(object, colors, "bg4")?,
        fg0: required_color(object, colors, "fg0")?,
        fg1: required_color(object, colors, "fg1")?,
        fg2: required_color(object, colors, "fg2")?,
        fg3: required_color(object, colors, "fg3")?,
        sel0: required_color(object, colors, "sel0")?,
        sel1: required_color(object, colors, "sel1")?,
        comment: required_color(object, colors, "comment")?,
        black: required_color(object, colors, "black")?,
        red: required_color(object, colors, "red")?,
        green: required_color(object, colors, "green")?,
        yellow: required_color(object, colors, "yellow")?,
        blue: required_color(object, colors, "blue")?,
        magenta: required_color(object, colors, "magenta")?,
        cyan: required_color(object, colors, "cyan")?,
        white: required_color(object, colors, "white")?,
        orange: required_color(object, colors, "orange")?,
        pink: required_color(object, colors, "pink")?,
    })
}

fn required_color(
    object: &serde_json::Map<String, Value>,
    colors: Option<&serde_json::Map<String, Value>>,
    field: &str,
) -> Result<String> {
    let value = colors
        .and_then(|colors| colors.get(field))
        .or_else(|| object.get(field))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("custom palette is missing required color field {field:?}"))?;

    validate_color_literal(field, value)?;
    Ok(value.to_string())
}

fn optional_string(object: &serde_json::Map<String, Value>, field: &str) -> Result<Option<String>> {
    match object.get(field) {
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => bail!("custom palette field {field:?} must be a string"),
        None => Ok(None),
    }
}

fn optional_bool(object: &serde_json::Map<String, Value>, field: &str) -> Result<Option<bool>> {
    match object.get(field) {
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => bail!("custom palette field {field:?} must be a boolean"),
        None => Ok(None),
    }
}

fn validate_color_literal(field: &str, value: &str) -> Result<()> {
    let valid_hex = value.len() == 7
        && value.starts_with('#')
        && value[1..].chars().all(|ch| ch.is_ascii_hexdigit());
    if valid_hex {
        Ok(())
    } else {
        bail!("custom palette color {field:?} must be a #RRGGBB hex color, got {value:?}")
    }
}

fn custom_palette_file(cfg: &ThemeConfig, key: &str) -> Result<Option<PathBuf>> {
    if !is_theme_key_safe(key) {
        return Ok(None);
    }
    Ok(Some(theme_dir_for_config(cfg)?.join(format!("{key}.json"))))
}

fn is_theme_key_safe(key: &str) -> bool {
    !key.is_empty()
        && !key.starts_with('.')
        && key
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
}

pub fn read_theme_config() -> Result<ThemeConfig> {
    let path = config_file()?;
    let mut cfg = ThemeConfig::default();
    let Some(text) = read_optional_text(&path)? else {
        return Ok(cfg);
    };
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
            "LUMA_THEME_DIR" => {
                cfg.theme_dir = non_empty(value).map(|path| expand_home_path(&path))
            }
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
    text.push_str(
        "# Color scheme keys select built-in palettes or JSON files in LUMA_THEME_DIR.\n",
    );
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
    if let Some(theme_dir) = &cfg.theme_dir {
        text.push_str(&format!(
            "LUMA_THEME_DIR={}\n",
            shell_quote(&theme_dir.to_string_lossy())
        ));
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
    if let Some(theme_dir) = &cfg.theme_dir {
        println!("LUMA_THEME_DIR={}", theme_dir.display());
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

pub fn read_optional_text(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to read {}", path.display())),
    }
}

pub fn write_if_changed(path: &Path, content: &str) -> Result<()> {
    match fs::read(path) {
        Ok(existing) if existing == content.as_bytes() => return Ok(()),
        Ok(_) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| format!("failed to read {}", path.display()));
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::write(path, content)
                .with_context(|| format!("failed to write {}", path.display()))?;
            return Ok(());
        }
        Ok(_) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| format!("failed to stat {}", path.display()));
        }
    }
    write_atomic(path, content.as_bytes())?;
    Ok(())
}

fn write_atomic(path: &Path, content: &[u8]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "luma".into());
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let tmp = parent.join(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        unique
    ));

    fs::write(&tmp, content).with_context(|| format!("failed to write {}", tmp.display()))?;
    match fs::metadata(path) {
        Ok(metadata) => {
            if let Err(err) = fs::set_permissions(&tmp, metadata.permissions()) {
                let _ = fs::remove_file(&tmp);
                return Err(err)
                    .with_context(|| format!("failed to set permissions on {}", tmp.display()));
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            let _ = fs::remove_file(&tmp);
            return Err(err).with_context(|| format!("failed to stat {}", path.display()));
        }
    }
    if let Err(err) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(err).with_context(|| format!("failed to replace {}", path.display()));
    }
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

pub fn default_theme_dir() -> Result<PathBuf> {
    Ok(config_dir()?.join("themes"))
}

pub fn theme_dir_for_config(cfg: &ThemeConfig) -> Result<PathBuf> {
    if let Some(path) = env::var_os("LUMA_THEME_DIR") {
        return Ok(expand_home_path(&path.to_string_lossy()));
    }
    if let Some(path) = &cfg.theme_dir {
        return Ok(path.clone());
    }
    default_theme_dir()
}

pub fn expand_home_path(value: &str) -> PathBuf {
    if value == "~" {
        return home_dir().unwrap_or_else(|_| PathBuf::from(value));
    }
    if let Some(rest) = value.strip_prefix("~/")
        && let Ok(home) = home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(value)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_theme_dir(test_name: &str) -> Result<PathBuf> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let dir = env::temp_dir().join(format!("luma-{test_name}-{}-{nanos}", std::process::id()));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    fn palette_json(name: &str, red: &str, omit: Option<&str>) -> String {
        let fields = [
            ("bg0", "#101010"),
            ("bg1", "#111111"),
            ("bg2", "#222222"),
            ("bg3", "#333333"),
            ("bg4", "#444444"),
            ("fg0", "#f0f0f0"),
            ("fg1", "#f1f1f1"),
            ("fg2", "#f2f2f2"),
            ("fg3", "#f3f3f3"),
            ("sel0", "#555555"),
            ("sel1", "#666666"),
            ("comment", "#777777"),
            ("black", "#000000"),
            ("red", red),
            ("green", "#00aa00"),
            ("yellow", "#aaaa00"),
            ("blue", "#0000aa"),
            ("magenta", "#aa00aa"),
            ("cyan", "#00aaaa"),
            ("white", "#ffffff"),
            ("orange", "#ffaa00"),
            ("pink", "#ff00aa"),
        ];
        let colors = fields
            .into_iter()
            .filter(|(field, _)| Some(*field) != omit)
            .map(|(field, value)| format!(r#"    "{field}": "{value}""#))
            .collect::<Vec<_>>()
            .join(",\n");
        format!(
            r#"{{
  "$schema": "https://raw.githubusercontent.com/rushrs/luma/main/schemas/palette.schema.json",
  "name": "{name}",
  "light": false,
  "colors": {{
{colors}
  }}
}}
"#
        )
    }

    #[test]
    fn custom_palette_overrides_builtin_by_key() -> Result<()> {
        let dir = temp_theme_dir("custom-overrides")?;
        fs::write(
            dir.join("carbonfox.json"),
            palette_json("Custom Carbonfox", "#112233", None),
        )?;
        let cfg = ThemeConfig {
            theme_dir: Some(dir.clone()),
            ..ThemeConfig::default()
        };

        let palette = palette_for_config(Mode::Dark, "carbonfox", &cfg)?;

        assert_eq!(palette.key, "carbonfox");
        assert_eq!(palette.name, "Custom Carbonfox");
        assert_eq!(palette.red, "#112233");
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn invalid_custom_palette_reports_missing_required_field() -> Result<()> {
        let dir = temp_theme_dir("missing-field")?;
        fs::write(
            dir.join("broken.json"),
            palette_json("Broken", "#112233", Some("pink")),
        )?;
        let cfg = ThemeConfig {
            theme_dir: Some(dir.clone()),
            ..ThemeConfig::default()
        };

        let err = palette_for_config(Mode::Dark, "broken", &cfg).unwrap_err();

        assert!(format!("{err:#}").contains("missing required color field \"pink\""));
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn available_palette_names_includes_custom_palettes() -> Result<()> {
        let dir = temp_theme_dir("palette-names")?;
        fs::write(
            dir.join("my-dark.json"),
            palette_json("My Dark", "#112233", None),
        )?;
        let cfg = ThemeConfig {
            theme_dir: Some(dir.clone()),
            ..ThemeConfig::default()
        };

        let names = available_palette_names(&cfg)?;

        assert!(names.contains("carbonfox"));
        assert!(names.contains("my-dark"));
        fs::remove_dir_all(dir)?;
        Ok(())
    }

    #[test]
    fn shell_quote_round_trips_single_quotes_for_config_parser() {
        let value = "dawn fox's bright";
        let quoted = shell_quote(value);

        assert_eq!(parse_config_value(&quoted), value);
    }

    #[test]
    fn validate_relative_path_rejects_escape_paths() {
        assert!(validate_relative_path(Path::new("themes/luma.json")).is_ok());
        assert!(validate_relative_path(Path::new("../outside")).is_err());
        assert!(validate_relative_path(Path::new("themes/../../outside")).is_err());
        assert!(validate_relative_path(Path::new("/tmp/outside")).is_err());
    }

    #[test]
    fn write_if_changed_compares_bytes_without_rewriting() -> Result<()> {
        let dir = temp_theme_dir("unchanged")?;
        let path = dir.join("file");
        fs::write(&path, b"same")?;
        let before = fs::metadata(&path)?.modified()?;

        write_if_changed(&path, "same")?;

        let after = fs::metadata(&path)?.modified()?;
        fs::remove_dir_all(dir)?;
        assert_eq!(before, after);
        Ok(())
    }

    #[test]
    fn read_optional_text_only_defaults_missing_files() -> Result<()> {
        let dir = temp_theme_dir("optional-read")?;
        let missing = dir.join("missing");
        assert!(read_optional_text(&missing)?.is_none());

        let invalid = dir.join("invalid-utf8");
        fs::write(&invalid, [0xff, 0xfe])?;
        let err = read_optional_text(&invalid).expect_err("invalid UTF-8 should be an error");
        fs::remove_dir_all(dir)?;
        assert!(err.to_string().contains("failed to read"));
        Ok(())
    }
}
