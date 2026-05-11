use std::fs;

use anyhow::Result;
use std::path::Path;

use luma_core::{AppId, LumaPlugin, Platform, SyncContext, TerminalEditor, write_if_changed};

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
    let luma_lua = platform
        .app_config_files(AppId::Nvim, Path::new("lua/luma.lua"))?
        .into_iter()
        .next()
        .expect("nvim config path exists");
    if let Some(parent) = luma_lua.parent() {
        fs::create_dir_all(parent)?;
    }
    write_if_changed(&luma_lua, NVIM_LUMA_LUA)?;

    let polish = platform
        .app_config_files(AppId::Nvim, Path::new("lua/polish.lua"))?
        .into_iter()
        .next()
        .expect("nvim polish path exists");
    let require_line = "pcall(require, \"luma\")";
    if polish.exists() {
        let text = fs::read_to_string(&polish)?;
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
