// SPDX-License-Identifier: MIT
use std::{fs, path::Path, path::PathBuf};

use anyhow::{Context, Result};
use luma_core::{
    AgenticHarness, AppId, LumaPlugin, Palette, Platform, SyncContext, THEME_NAME,
    primary_app_config_file, read_optional_text, write_if_changed,
};
use serde_json::{Value, json};

#[derive(Debug, Default)]
pub struct Pi;

impl LumaPlugin for Pi {
    fn name(&self) -> &'static str {
        "pi"
    }

    fn sync(&self, ctx: &SyncContext) -> Result<()> {
        write_pi_theme(ctx)?;
        select_pi_theme(ctx)?;
        Ok(())
    }
}

impl AgenticHarness for Pi {}

pub fn pi_theme_file(platform: &dyn Platform) -> Result<PathBuf> {
    primary_app_config_file(
        platform,
        AppId::Pi,
        Path::new(&format!("themes/{THEME_NAME}.json")),
    )
}

pub fn pi_settings_file(platform: &dyn Platform) -> Result<PathBuf> {
    primary_app_config_file(platform, AppId::Pi, Path::new("settings.json"))
}

fn write_pi_theme(ctx: &SyncContext) -> Result<()> {
    let path = pi_theme_file(ctx.platform)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let theme = render_pi_theme(&ctx.palette);
    let text = serde_json::to_string_pretty(&theme)? + "\n";
    write_if_changed(&path, &text)
}

fn select_pi_theme(ctx: &SyncContext) -> Result<()> {
    let path = pi_settings_file(ctx.platform)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut settings: Value = match read_optional_text(&path)? {
        Some(text) => serde_json::from_str(&text)
            .with_context(|| format!("failed to parse Pi settings JSON at {}", path.display()))?,
        None => json!({}),
    };
    if !settings.is_object() {
        settings = json!({});
    }
    settings["theme"] = json!(THEME_NAME);
    let text = serde_json::to_string_pretty(&settings)? + "\n";
    write_if_changed(&path, &text)
}

fn render_pi_theme(p: &Palette) -> Value {
    let tool_success_bg = if p.light { &p.sel0 } else { &p.bg2 };
    let tool_error_bg = if p.light { &p.bg0 } else { &p.bg2 };
    json!({
        "$schema": "https://raw.githubusercontent.com/earendil-works/pi-mono/main/packages/coding-agent/src/modes/interactive/theme/theme-schema.json",
        "name": THEME_NAME,
        "vars": {
            "bg": p.bg1,
            "bg0": p.bg0,
            "bg2": p.bg2,
            "bg3": p.bg3,
            "border": p.bg4,
            "fg": p.fg1,
            "fg2": p.fg2,
            "muted": p.comment,
            "selected": p.sel0,
            "selectedStrong": p.sel1,
            "blue": p.blue,
            "cyan": p.cyan,
            "green": p.green,
            "yellow": p.yellow,
            "orange": p.orange,
            "red": p.red,
            "magenta": p.magenta,
            "pink": p.pink,
            "white": p.white,
            "black": p.black
        },
        "colors": {
            "accent": "blue",
            "border": "border",
            "borderAccent": "blue",
            "borderMuted": "muted",
            "success": "green",
            "error": "red",
            "warning": "yellow",
            "muted": "muted",
            "dim": "fg2",
            "text": "fg",
            "thinkingText": "muted",
            "selectedBg": "selected",
            "userMessageBg": "bg2",
            "userMessageText": "fg",
            "customMessageBg": "bg2",
            "customMessageText": "fg",
            "customMessageLabel": "magenta",
            "toolPendingBg": "bg0",
            "toolSuccessBg": tool_success_bg,
            "toolErrorBg": tool_error_bg,
            "toolTitle": "blue",
            "toolOutput": "fg",
            "mdHeading": "orange",
            "mdLink": "blue",
            "mdLinkUrl": "cyan",
            "mdCode": "pink",
            "mdCodeBlock": "fg",
            "mdCodeBlockBorder": "border",
            "mdQuote": "muted",
            "mdQuoteBorder": "muted",
            "mdHr": "border",
            "mdListBullet": "cyan",
            "toolDiffAdded": "green",
            "toolDiffRemoved": "red",
            "toolDiffContext": "muted",
            "syntaxComment": "muted",
            "syntaxKeyword": "magenta",
            "syntaxFunction": "blue",
            "syntaxVariable": "white",
            "syntaxString": "green",
            "syntaxNumber": "orange",
            "syntaxType": "yellow",
            "syntaxOperator": "cyan",
            "syntaxPunctuation": "fg2",
            "thinkingOff": "muted",
            "thinkingMinimal": "blue",
            "thinkingLow": "cyan",
            "thinkingMedium": "yellow",
            "thinkingHigh": "magenta",
            "thinkingXhigh": "red",
            "bashMode": "orange"
        },
        "export": {
            "pageBg": p.bg1,
            "cardBg": p.bg2,
            "infoBg": p.bg0
        }
    })
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
            let path = std::env::temp_dir().join(format!(
                "luma-harnesses-{name}-{}-{unique}",
                std::process::id()
            ));
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
    }

    impl Platform for FakePlatform {
        fn os_kind(&self) -> OsKind {
            OsKind::Unknown
        }

        fn home_dir(&self) -> Result<PathBuf> {
            Ok(self.root.clone())
        }

        fn app_config_files(&self, app: AppId, relative: &Path) -> Result<Vec<PathBuf>> {
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
    fn invalid_pi_settings_json_is_not_overwritten() -> Result<()> {
        let temp = TempDir::new("invalid-json")?;
        let platform = FakePlatform {
            root: temp.0.clone(),
        };
        let settings_path = pi_settings_file(&platform)?;
        fs::create_dir_all(settings_path.parent().expect("settings path has a parent"))?;
        fs::write(&settings_path, "{not valid json")?;
        let ctx = SyncContext::new(Mode::Dark, ThemeConfig::default(), &platform)?;

        let err = select_pi_theme(&ctx).expect_err("invalid JSON should fail closed");

        assert!(err.to_string().contains("failed to parse Pi settings JSON"));
        assert_eq!(fs::read_to_string(settings_path)?, "{not valid json");
        Ok(())
    }

    #[test]
    fn pi_settings_object_gets_theme_selected() -> Result<()> {
        let temp = TempDir::new("select-theme")?;
        let platform = FakePlatform {
            root: temp.0.clone(),
        };
        let settings_path = pi_settings_file(&platform)?;
        fs::create_dir_all(settings_path.parent().expect("settings path has a parent"))?;
        fs::write(&settings_path, r#"{"other":true}"#)?;
        let ctx = SyncContext::new(Mode::Dark, ThemeConfig::default(), &platform)?;

        select_pi_theme(&ctx)?;

        let settings: Value = serde_json::from_str(&fs::read_to_string(settings_path)?)?;
        assert_eq!(settings["theme"], json!(THEME_NAME));
        assert_eq!(settings["other"], json!(true));
        Ok(())
    }
}
