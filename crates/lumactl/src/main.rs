// SPDX-License-Identifier: MIT
use std::{env, fs, path::Path, process::Command, sync::mpsc, thread, time::Duration};

use anyhow::{Result, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use luma_core::{
    AppId, AppearanceBackend, DesiredMode, LAUNCH_LABEL, LumaPlugin, SyncContext, THEME_NAME,
    ThemeConfig, TmuxMode, config_file, parse_plugin_list, print_theme_config, read_theme_config,
    supported_palette_names, write_theme_config,
};
use luma_editors::{Nvim, install_nvim_integration};
use luma_harnesses::{Pi, pi_settings_file, pi_theme_file};
use luma_os_macos::{
    MacOs, current_uid, launch_agent_file, reload_launch_agent, run_appearance_notification_loop,
    unload_launch_agent, write_launch_agent,
};
use luma_terminals::{Ghostty, Tmux as TmuxPlugin, tmux_config_file, tmux_theme_file};
use luma_tui::{K9s, k9s_config_file, k9s_skin_file};

#[derive(Parser, Debug)]
#[command(name = "lumactl")]
#[command(
    about = "Coordinate light/dark themes across terminals, TUIs, editors, and agent harnesses"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Install or upgrade lumactl, launchd watcher, and configured app integrations.
    Install(InstallArgs),
    /// Synchronize configured plugins with the current OS appearance.
    Sync,
    /// Toggle or force the OS appearance, then sync configured plugins.
    Toggle(ToggleArgs),
    /// Force dark mode, then sync configured plugins.
    Dark,
    /// Force light mode, then sync configured plugins.
    Light,
    /// Run the long-lived watcher used by launchd.
    Watch,
    /// Unload the watcher and remove Luma-managed files/blocks.
    Uninstall,
    /// Show current state and LaunchAgent status.
    Status,
    /// Show or update theme/plugin choices.
    Config(ConfigArgs),
    /// List built-in palettes used by generated-theme plugins.
    Palettes,
    /// List built-in plugins that can be enabled in LUMA_PLUGINS.
    Plugins,
}

#[derive(Args, Debug)]
struct InstallArgs {
    /// Light-mode colorscheme key.
    #[arg(long)]
    light: Option<String>,

    /// Dark-mode colorscheme key.
    #[arg(long)]
    dark: Option<String>,

    /// Ghostty light theme display name, if different from the colorscheme key.
    #[arg(long = "light-ghostty")]
    light_ghostty: Option<String>,

    /// Ghostty dark theme display name, if different from the colorscheme key.
    #[arg(long = "dark-ghostty")]
    dark_ghostty: Option<String>,

    /// Built-in plugins to enable, comma-separated. Available: nvim,ghostty,tmux,k9s,pi.
    #[arg(long)]
    plugins: Option<String>,

    /// Tmux integration depth: palette, statusline, or off.
    #[arg(long = "tmux-mode", value_enum)]
    tmux_mode: Option<TmuxModeArg>,

    /// Do not install the Neovim module / polish.lua require.
    #[arg(long)]
    no_nvim: bool,
}

#[derive(Args, Debug)]
struct ToggleArgs {
    /// Optional target mode. Defaults to toggling the current OS mode.
    #[arg(value_enum, default_value_t = ToggleMode::Toggle)]
    mode: ToggleMode,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ToggleMode {
    Toggle,
    Dark,
    Light,
}

impl From<ToggleMode> for DesiredMode {
    fn from(value: ToggleMode) -> Self {
        match value {
            ToggleMode::Toggle => Self::Toggle,
            ToggleMode::Dark => Self::Dark,
            ToggleMode::Light => Self::Light,
        }
    }
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum TmuxModeArg {
    Palette,
    Statusline,
    Off,
}

impl From<TmuxModeArg> for TmuxMode {
    fn from(value: TmuxModeArg) -> Self {
        match value {
            TmuxModeArg::Palette => Self::Palette,
            TmuxModeArg::Statusline => Self::Statusline,
            TmuxModeArg::Off => Self::Off,
        }
    }
}

#[derive(Args, Debug)]
struct ConfigArgs {
    /// Light-mode colorscheme key.
    #[arg(long)]
    light: Option<String>,

    /// Dark-mode colorscheme key.
    #[arg(long)]
    dark: Option<String>,

    /// Ghostty light theme display name, if different from the colorscheme key.
    #[arg(long = "light-ghostty")]
    light_ghostty: Option<String>,

    /// Ghostty dark theme display name, if different from the colorscheme key.
    #[arg(long = "dark-ghostty")]
    dark_ghostty: Option<String>,

    /// Built-in plugins to enable, comma-separated. Available: nvim,ghostty,tmux,k9s,pi.
    #[arg(long)]
    plugins: Option<String>,

    /// Tmux integration depth: palette, statusline, or off.
    #[arg(long = "tmux-mode", value_enum)]
    tmux_mode: Option<TmuxModeArg>,

    /// Print effective config after applying updates.
    #[arg(long)]
    show: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let os = MacOs;
    match cli.command.unwrap_or(Commands::Sync) {
        Commands::Install(args) => install(&os, args),
        Commands::Sync => sync_all(&os).map(|_| ()),
        Commands::Toggle(args) => toggle(&os, args.mode.into()),
        Commands::Dark => toggle(&os, DesiredMode::Dark),
        Commands::Light => toggle(&os, DesiredMode::Light),
        Commands::Watch => watch(&os),
        Commands::Uninstall => uninstall(&os),
        Commands::Status => status(&os),
        Commands::Config(args) => config_command(&os, args),
        Commands::Palettes => {
            println!("{}", supported_palette_names());
            Ok(())
        }
        Commands::Plugins => {
            println!("{}", available_plugin_names().join(", "));
            Ok(())
        }
    }
}

fn install(os: &impl AppearanceBackend, args: InstallArgs) -> Result<()> {
    let mut cfg = read_theme_config()?;
    let config_changed = apply_config_args(
        &mut cfg,
        args.light,
        args.dark,
        args.light_ghostty,
        args.dark_ghostty,
        args.plugins,
        args.tmux_mode,
    );
    if config_changed || !config_file()?.exists() {
        write_theme_config(&cfg)?;
    }

    luma_os_macos::install_binary(&env::current_exe()?)?;
    if !args.no_nvim && cfg.plugins.iter().any(|plugin| plugin == "nvim") {
        install_nvim_integration(os)?;
    }
    write_launch_agent()?;
    sync_all(os)?;
    reload_launch_agent()?;

    println!(
        "Installed lumactl.\n\nConfig:\n  {}\n\nManual controls:\n  lumactl toggle\n  lumactl dark\n  lumactl light\n\nWatcher status:\n  launchctl print gui/$(id -u)/{}",
        config_file()?.display(),
        LAUNCH_LABEL,
    );
    Ok(())
}

fn uninstall(os: &impl AppearanceBackend) -> Result<()> {
    unload_launch_agent()?;
    remove_file_if_exists(&launch_agent_file()?)?;

    remove_nvim_integration(os)?;
    remove_ghostty_integration(os)?;
    remove_tmux_integration(os)?;
    remove_k9s_integration(os)?;
    remove_pi_integration(os)?;
    remove_luma_cache(os)?;

    println!("Uninstalled Luma managed files and unloaded {LAUNCH_LABEL}.");
    println!("Kept config at {}", config_file()?.display());
    Ok(())
}

fn remove_nvim_integration(platform: &dyn luma_core::Platform) -> Result<()> {
    let luma_lua = platform
        .app_config_files(AppId::Nvim, Path::new("lua/luma.lua"))?
        .into_iter()
        .next()
        .expect("nvim luma.lua path exists");
    remove_file_if_exists(&luma_lua)?;

    let polish = platform
        .app_config_files(AppId::Nvim, Path::new("lua/polish.lua"))?
        .into_iter()
        .next()
        .expect("nvim polish path exists");
    remove_lines_if_exists(&polish, |line| {
        line.trim() == "pcall(require, \"luma\")"
            || line.trim() == "-- macOS light/dark theme sync. Installed by Luma."
    })?;
    Ok(())
}

fn remove_ghostty_integration(platform: &dyn luma_core::Platform) -> Result<()> {
    let candidates = platform.app_config_files(AppId::Ghostty, Path::new("config"))?;
    for path in candidates {
        remove_lines_if_exists(&path, |line| {
            let line = line.trim_start();
            line.starts_with("theme = light:") && line.contains(",dark:")
        })?;
    }
    Ok(())
}

fn remove_tmux_integration(platform: &dyn luma_core::Platform) -> Result<()> {
    remove_file_if_exists(&tmux_theme_file(platform)?)?;
    let config = tmux_config_file(platform)?;
    if let Ok(text) = fs::read_to_string(&config) {
        let next = remove_marked_block(&text, "# >>> luma", "# <<< luma");
        if next != text {
            write_text_or_remove(&config, &next)?;
        }
    }
    Ok(())
}

fn remove_k9s_integration(platform: &dyn luma_core::Platform) -> Result<()> {
    remove_file_if_exists(&k9s_skin_file(platform, THEME_NAME)?)?;
    let config = k9s_config_file(platform)?;
    remove_lines_if_exists(&config, |line| line.trim() == "skin: luma")?;
    if let Ok(text) = fs::read_to_string(&config)
        && text.trim() == "k9s:\n  ui:\n    reactive: true"
    {
        remove_file_if_exists(&config)?;
    }
    Ok(())
}

fn remove_pi_integration(platform: &dyn luma_core::Platform) -> Result<()> {
    remove_file_if_exists(&pi_theme_file(platform)?)?;
    let settings = pi_settings_file(platform)?;
    if let Ok(text) = fs::read_to_string(&settings)
        && let Ok(mut value) = serde_json::from_str::<serde_json::Value>(&text)
        && value.get("theme").and_then(|theme| theme.as_str()) == Some(THEME_NAME)
        && let Some(object) = value.as_object_mut()
    {
        object.remove("theme");
        let next = if object.is_empty() {
            String::new()
        } else {
            serde_json::to_string_pretty(&value)? + "\n"
        };
        write_text_or_remove(&settings, &next)?;
    }
    Ok(())
}

fn remove_luma_cache(platform: &dyn luma_core::Platform) -> Result<()> {
    for relative in ["mode", "nvim-colorscheme"] {
        let path = platform.app_cache_file(AppId::Luma, Path::new(relative))?;
        remove_file_if_exists(&path)?;
    }
    Ok(())
}

fn remove_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

fn remove_lines_if_exists(path: &Path, should_remove: impl Fn(&str) -> bool) -> Result<()> {
    let Ok(text) = fs::read_to_string(path) else {
        return Ok(());
    };
    let kept: Vec<&str> = text.lines().filter(|line| !should_remove(line)).collect();
    let next = if kept.is_empty() {
        String::new()
    } else {
        format!("{}\n", kept.join("\n"))
    };
    if next != text {
        write_text_or_remove(path, &next)?;
    }
    Ok(())
}

fn remove_marked_block(text: &str, start: &str, end: &str) -> String {
    let Some(start_idx) = text.find(start) else {
        return text.to_string();
    };
    let Some(relative_end_idx) = text[start_idx..].find(end) else {
        return text.to_string();
    };
    let end_idx = start_idx + relative_end_idx + end.len();
    let mut next = String::new();
    next.push_str(&text[..start_idx]);
    if let Some(suffix) = text.get(end_idx..) {
        next.push_str(suffix.trim_start_matches('\n'));
    }
    while next.contains("\n\n\n") {
        next = next.replace("\n\n\n", "\n\n");
    }
    if !next.is_empty() && !next.ends_with('\n') {
        next.push('\n');
    }
    next
}

fn write_text_or_remove(path: &Path, text: &str) -> Result<()> {
    if text.trim().is_empty() {
        remove_file_if_exists(path)
    } else {
        fs::write(path, text)?;
        Ok(())
    }
}

fn sync_all<'a>(os: &'a impl AppearanceBackend) -> Result<SyncContext<'a>> {
    let cfg = read_theme_config()?;
    let mode = os.current_mode()?;
    let ctx = SyncContext::new(mode, cfg, os)?;

    for plugin in selected_plugins(&ctx.config.plugins)? {
        plugin.sync(&ctx)?;
    }

    println!(
        "lumactl: {} ({}) plugins={}",
        ctx.mode.as_str(),
        ctx.theme,
        ctx.config.plugins.join(",")
    );
    Ok(ctx)
}

fn toggle(os: &impl AppearanceBackend, mode: DesiredMode) -> Result<()> {
    os.set_mode(mode)?;
    sync_all(os)?;
    Ok(())
}

fn watch(_os: &impl AppearanceBackend) -> Result<()> {
    let (tx, rx) = mpsc::channel();
    let poll = watch_poll_interval();

    thread::spawn(move || {
        let os = MacOs;
        let mut last_mode = match sync_all(&os) {
            Ok(ctx) => ctx.mode,
            Err(err) => {
                eprintln!("lumactl: initial sync failed: {err}");
                return;
            }
        };

        loop {
            match rx.recv_timeout(poll) {
                Ok(()) => {
                    eprintln!("lumactl: native appearance notification");
                    match os.current_mode() {
                        Ok(mode) if mode != last_mode => match sync_all(&os) {
                            Ok(ctx) => last_mode = ctx.mode,
                            Err(err) => eprintln!("lumactl: sync failed: {err}"),
                        },
                        Ok(_) => {}
                        Err(err) => eprintln!("lumactl: failed to read appearance: {err}"),
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => match os.current_mode() {
                    Ok(mode) if mode != last_mode => match sync_all(&os) {
                        Ok(ctx) => last_mode = ctx.mode,
                        Err(err) => eprintln!("lumactl: sync failed: {err}"),
                    },
                    Ok(_) => {}
                    Err(err) => eprintln!("lumactl: failed to read appearance: {err}"),
                },
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
    });

    eprintln!(
        "lumactl: watching native macOS appearance notifications (fallback_poll={}ms)",
        poll.as_millis()
    );
    run_appearance_notification_loop(tx)
}

fn watch_poll_interval() -> Duration {
    const DEFAULT_MS: u64 = 1_000;
    let ms = env::var("LUMA_WATCH_POLL_MS")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok())
        .filter(|ms| (50..=5_000).contains(ms))
        .unwrap_or(DEFAULT_MS);
    Duration::from_millis(ms)
}

fn status(os: &impl AppearanceBackend) -> Result<()> {
    let cfg = read_theme_config()?;
    let mode = os.current_mode()?;
    let ctx = SyncContext::new(mode, cfg, os)?;
    let caps = os.capabilities();

    println!("os: {}", os.name());
    println!("mode: {}", mode.as_str());
    println!("active-theme: {}", ctx.theme);
    println!("light-theme: {}", ctx.config.light);
    println!("dark-theme: {}", ctx.config.dark);
    println!("plugins: {}", ctx.config.plugins.join(","));
    println!("tmux-mode: {}", ctx.config.tmux_mode.as_str());
    println!("config: {}", config_file()?.display());
    println!("tmux-theme: {}", tmux_theme_file(ctx.platform)?.display());
    println!(
        "k9s-skin: {}",
        k9s_skin_file(ctx.platform, luma_core::THEME_NAME)?.display()
    );
    println!(
        "pi-theme: {}",
        luma_harnesses::pi_theme_file(ctx.platform)?.display()
    );
    println!("launch-agent: {}", launch_agent_file()?.display());
    println!(
        "capabilities: read={} set={} watch={}",
        caps.can_read, caps.can_set, caps.can_watch
    );

    let uid = current_uid()?;
    let output = Command::new("launchctl")
        .args(["print", &format!("gui/{uid}/{LAUNCH_LABEL}")])
        .output();
    match output {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("state =") || trimmed.starts_with("pid =") {
                    println!("launchd-{trimmed}");
                }
            }
        }
        Ok(_) => println!("launchd: not loaded"),
        Err(err) => println!("launchd: unavailable ({err})"),
    }
    Ok(())
}

fn config_command(os: &impl AppearanceBackend, args: ConfigArgs) -> Result<()> {
    let mut cfg = read_theme_config()?;
    let changed = apply_config_args(
        &mut cfg,
        args.light,
        args.dark,
        args.light_ghostty,
        args.dark_ghostty,
        args.plugins,
        args.tmux_mode,
    );
    if changed {
        write_theme_config(&cfg)?;
        sync_all(os)?;
    }
    if args.show || !changed {
        print_theme_config(&cfg);
    }
    Ok(())
}

fn apply_config_args(
    cfg: &mut ThemeConfig,
    light: Option<String>,
    dark: Option<String>,
    light_ghostty: Option<String>,
    dark_ghostty: Option<String>,
    plugins: Option<String>,
    tmux_mode: Option<TmuxModeArg>,
) -> bool {
    let mut changed = false;
    if let Some(light) = light {
        cfg.light = luma_core::normalize_theme_name(&light);
        changed = true;
    }
    if let Some(dark) = dark {
        cfg.dark = luma_core::normalize_theme_name(&dark);
        changed = true;
    }
    if let Some(light_ghostty) = light_ghostty {
        cfg.light_ghostty = Some(light_ghostty);
        changed = true;
    }
    if let Some(dark_ghostty) = dark_ghostty {
        cfg.dark_ghostty = Some(dark_ghostty);
        changed = true;
    }
    if let Some(plugins) = plugins {
        cfg.plugins = parse_plugin_list(&plugins);
        changed = true;
    }
    if let Some(tmux_mode) = tmux_mode {
        cfg.tmux_mode = tmux_mode.into();
        changed = true;
    }
    changed
}

fn available_plugin_names() -> Vec<&'static str> {
    vec!["nvim", "ghostty", "tmux", "k9s", "pi"]
}

fn selected_plugins(names: &[String]) -> Result<Vec<Box<dyn LumaPlugin>>> {
    let mut plugins: Vec<Box<dyn LumaPlugin>> = Vec::new();
    for name in names {
        match name.as_str() {
            "nvim" => plugins.push(Box::new(Nvim)),
            "ghostty" => plugins.push(Box::new(Ghostty)),
            "tmux" => plugins.push(Box::new(TmuxPlugin)),
            "k9s" => plugins.push(Box::new(K9s)),
            "pi" => plugins.push(Box::new(Pi)),
            unknown => bail!(
                "unknown plugin {unknown:?}; available plugins: {}",
                available_plugin_names().join(", ")
            ),
        }
    }
    Ok(plugins)
}
