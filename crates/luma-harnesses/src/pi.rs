// SPDX-License-Identifier: MIT
use std::{fs, path::Path, path::PathBuf};

use anyhow::Result;
use luma_core::{
    AgenticHarness, AppId, LumaPlugin, Palette, Platform, SyncContext, THEME_NAME, write_if_changed,
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
    Ok(platform
        .app_config_files(AppId::Pi, Path::new(&format!("themes/{THEME_NAME}.json")))?
        .into_iter()
        .next()
        .expect("pi theme path exists"))
}

pub fn pi_settings_file(platform: &dyn Platform) -> Result<PathBuf> {
    Ok(platform
        .app_config_files(AppId::Pi, Path::new("settings.json"))?
        .into_iter()
        .next()
        .expect("pi settings path exists"))
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
    let mut settings: Value = fs::read_to_string(&path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| json!({}));
    if !settings.is_object() {
        settings = json!({});
    }
    settings["theme"] = json!(THEME_NAME);
    let text = serde_json::to_string_pretty(&settings)? + "\n";
    write_if_changed(&path, &text)
}

fn render_pi_theme(p: &Palette) -> Value {
    let tool_success_bg = if p.light { p.sel0 } else { p.bg2 };
    let tool_error_bg = if p.light { p.bg0 } else { p.bg2 };
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
