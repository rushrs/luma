// SPDX-License-Identifier: MIT
use std::{fs, path::Path, path::PathBuf};

use anyhow::Result;
use luma_core::{AppId, LumaPlugin, SyncContext, Terminal, canonical_or_self, write_if_changed};

#[derive(Debug, Default)]
pub struct Ghostty;

impl LumaPlugin for Ghostty {
    fn name(&self) -> &'static str {
        "ghostty"
    }

    fn sync(&self, ctx: &SyncContext) -> Result<()> {
        update_ghostty_config(ctx)
    }
}

impl Terminal for Ghostty {}

fn update_ghostty_config(ctx: &SyncContext) -> Result<()> {
    let light = ctx.config.ghostty_theme_for_mode(luma_core::Mode::Light);
    let dark = ctx.config.ghostty_theme_for_mode(luma_core::Mode::Dark);
    let line = format!("theme = light:{light},dark:{dark}");

    let candidates = ctx
        .platform
        .app_config_files(AppId::Ghostty, Path::new("config"))?;
    let mut seen = Vec::new();
    for candidate in candidates {
        let path = canonical_or_self(&candidate);
        if seen.iter().any(|existing: &PathBuf| existing == &path) {
            continue;
        }
        seen.push(path.clone());
        update_theme_line(&path, &line)?;
    }
    Ok(())
}

fn update_theme_line(path: &Path, line: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = fs::read_to_string(path).unwrap_or_default();
    let mut lines: Vec<String> = text.lines().map(ToOwned::to_owned).collect();
    if let Some(existing) = lines
        .iter_mut()
        .find(|current| current.trim_start().starts_with("theme ="))
    {
        *existing = line.to_string();
    } else {
        if lines.last().is_some_and(|last| !last.is_empty()) {
            lines.push(String::new());
        }
        lines.push(line.to_string());
    }
    let next = format!("{}\n", lines.join("\n"));
    write_if_changed(path, &next)
}
