// SPDX-License-Identifier: MIT
use std::fs;

use anyhow::Result;
use std::path::Path;

use luma_core::{
    AppId, LumaPlugin, Platform, SyncContext, TerminalEditor, primary_app_config_file,
    read_optional_text, write_if_changed,
};

const NVIM_LUMA_LUA: &str = include_str!("nvim-luma.lua");

#[derive(Debug, Default)]
pub struct Nvim;

impl LumaPlugin for Nvim {
    fn name(&self) -> &'static str {
        "nvim"
    }

    fn sync(&self, ctx: &SyncContext) -> Result<()> {
        write_nvim_state(ctx)
    }
}

impl TerminalEditor for Nvim {}

fn write_nvim_state(ctx: &SyncContext) -> Result<()> {
    let mode_file = ctx
        .platform
        .app_cache_file(AppId::Luma, Path::new("mode"))?;
    let theme_file = ctx
        .platform
        .app_cache_file(AppId::Luma, Path::new("nvim-colorscheme"))?;
    if let Some(parent) = mode_file.parent() {
        fs::create_dir_all(parent)?;
    }
    write_if_changed(&mode_file, &format!("{}\n", ctx.mode.as_str()))?;
    write_if_changed(&theme_file, &format!("{}\n", ctx.theme))?;

    Ok(())
}

pub fn install_nvim_integration(platform: &dyn Platform) -> Result<()> {
    let luma_lua = primary_app_config_file(platform, AppId::Nvim, Path::new("lua/luma.lua"))?;
    if let Some(parent) = luma_lua.parent() {
        fs::create_dir_all(parent)?;
    }
    write_if_changed(&luma_lua, NVIM_LUMA_LUA)?;

    let polish = primary_app_config_file(platform, AppId::Nvim, Path::new("lua/polish.lua"))?;
    let require_line = "pcall(require, \"luma\")";
    if let Some(text) = read_optional_text(&polish)? {
        if text.contains(require_line) {
            return Ok(());
        }
        if text.contains("LumaSync") || text.contains("apply_theme_sync") {
            println!(
                "Existing Neovim theme sync detected in {}; not appending duplicate require.",
                polish.display()
            );
            return Ok(());
        }
        let next =
            format!("{text}\n-- macOS light/dark theme sync. Installed by Luma.\n{require_line}\n");
        write_if_changed(&polish, &next)?;
    } else {
        write_if_changed(
            &polish,
            &format!("-- macOS light/dark theme sync. Installed by Luma.\n{require_line}\n"),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use luma_core::OsKind;

    struct EmptyPlatform;

    impl Platform for EmptyPlatform {
        fn os_kind(&self) -> OsKind {
            OsKind::Unknown
        }

        fn home_dir(&self) -> Result<std::path::PathBuf> {
            Ok(std::path::PathBuf::new())
        }

        fn app_config_files(
            &self,
            _app: AppId,
            _relative: &Path,
        ) -> Result<Vec<std::path::PathBuf>> {
            Ok(Vec::new())
        }

        fn app_cache_file(&self, _app: AppId, relative: &Path) -> Result<std::path::PathBuf> {
            Ok(relative.to_path_buf())
        }
    }

    #[test]
    fn install_nvim_integration_returns_error_when_platform_has_no_config_path() {
        let err = install_nvim_integration(&EmptyPlatform)
            .expect_err("empty platform paths should be an error, not a panic");

        assert!(err.to_string().contains("returned no config file"));
    }
}
