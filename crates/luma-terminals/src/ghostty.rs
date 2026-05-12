// SPDX-License-Identifier: MIT
use std::{fs, path::Path, path::PathBuf};

use anyhow::{Result, bail};
use luma_core::{
    AppId, LumaPlugin, SyncContext, Terminal, canonical_or_self, read_optional_text,
    write_if_changed,
};

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
    validate_ghostty_theme_component(&light)?;
    validate_ghostty_theme_component(&dark)?;
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
    let text = read_optional_text(path)?.unwrap_or_default();
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

fn validate_ghostty_theme_component(value: &str) -> Result<()> {
    if value.trim().is_empty() || value.contains(['\n', '\r', ',']) {
        bail!(
            "Ghostty theme names must be non-empty and must not contain newlines or commas: {value:?}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is before UNIX_EPOCH")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "luma-ghostty-{name}-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn update_theme_line_replaces_existing_theme_only() -> Result<()> {
        let path = temp_path("replace");
        fs::write(&path, "font-size = 14\ntheme = old\nwindow-padding-x = 8\n")?;

        update_theme_line(&path, "theme = light:Dawnfox,dark:Carbonfox")?;

        let next = fs::read_to_string(&path)?;
        fs::remove_file(path)?;
        assert_eq!(
            next,
            "font-size = 14\ntheme = light:Dawnfox,dark:Carbonfox\nwindow-padding-x = 8\n"
        );
        Ok(())
    }

    #[test]
    fn invalid_ghostty_config_utf8_is_not_overwritten() -> Result<()> {
        let path = temp_path("invalid-utf8");
        fs::write(&path, [0xff, 0xfe])?;

        let err = update_theme_line(&path, "theme = light:Dawnfox,dark:Carbonfox")
            .expect_err("invalid UTF-8 should fail closed");

        assert!(err.to_string().contains("failed to read"));
        assert_eq!(fs::read(&path)?, [0xff, 0xfe]);
        fs::remove_file(path)?;
        Ok(())
    }

    #[test]
    fn ghostty_theme_component_validation_rejects_config_breakers() {
        assert!(validate_ghostty_theme_component("Dawnfox").is_ok());
        assert!(validate_ghostty_theme_component("").is_err());
        assert!(validate_ghostty_theme_component("bad\nname").is_err());
        assert!(validate_ghostty_theme_component("bad,name").is_err());
    }
}
