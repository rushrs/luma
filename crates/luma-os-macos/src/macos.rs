// SPDX-License-Identifier: MIT
use std::{
    ffi::{CString, c_char, c_void},
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{OnceLock, mpsc::Sender},
};

use anyhow::{Context, Result, anyhow, bail};
use luma_core::{
    AppId, AppearanceBackend, AppearanceCapabilities, DesiredMode, LAUNCH_LABEL, Mode, OsKind,
    Platform, home_dir, local_bin_dir, write_if_changed,
};

type Id = *mut c_void;
type Class = *mut c_void;
type Sel = *mut c_void;
type Imp = *const c_void;

const APPEARANCE_NOTIFICATION: &str = "AppleInterfaceThemeChangedNotification";
static APPEARANCE_SENDER: OnceLock<Sender<()>> = OnceLock::new();

#[link(name = "Foundation", kind = "framework")]
unsafe extern "C" {
    static NSFoundationVersionNumber: f64;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRunLoopRun();
}

#[link(name = "objc")]
unsafe extern "C" {
    fn objc_getClass(name: *const c_char) -> Class;
    fn objc_allocateClassPair(superclass: Class, name: *const c_char, extra_bytes: usize) -> Class;
    fn objc_registerClassPair(cls: Class);
    fn class_addMethod(cls: Class, name: Sel, imp: Imp, types: *const c_char) -> c_char;
    fn class_createInstance(cls: Class, extra_bytes: usize) -> Id;
    fn sel_registerName(name: *const c_char) -> Sel;
    fn objc_msgSend();
}

#[derive(Debug, Default)]
pub struct MacOs;

impl Platform for MacOs {
    fn os_kind(&self) -> OsKind {
        OsKind::MacOs
    }

    fn home_dir(&self) -> Result<PathBuf> {
        home_dir()
    }

    fn app_config_files(&self, app: AppId, relative: &Path) -> Result<Vec<PathBuf>> {
        let home = home_dir()?;
        let files = match app {
            AppId::Luma => vec![home.join(".config/luma").join(relative)],
            AppId::Nvim => vec![home.join(".config/nvim").join(relative)],
            AppId::Ghostty => vec![
                home.join(".config/ghostty").join(relative),
                home.join("Library/Application Support/com.mitchellh.ghostty")
                    .join(relative),
            ],
            AppId::Tmux if relative == Path::new("tmux.conf") => vec![home.join(".tmux.conf")],
            AppId::Tmux => vec![home.join(".tmux").join(relative)],
            AppId::K9s => vec![home.join("Library/Application Support/k9s").join(relative)],
            AppId::Pi => vec![home.join(".pi/agent").join(relative)],
        };
        Ok(files)
    }

    fn app_cache_file(&self, app: AppId, relative: &Path) -> Result<PathBuf> {
        let home = home_dir()?;
        let base = match app {
            AppId::Luma => home.join(".cache/luma"),
            AppId::Nvim => home.join(".cache/nvim"),
            AppId::Ghostty => home.join(".cache/ghostty"),
            AppId::Tmux => home.join(".cache/tmux"),
            AppId::K9s => home.join("Library/Application Support/k9s"),
            AppId::Pi => home.join(".cache/pi"),
        };
        Ok(base.join(relative))
    }
}

impl AppearanceBackend for MacOs {
    fn name(&self) -> &'static str {
        "macos"
    }

    fn capabilities(&self) -> AppearanceCapabilities {
        AppearanceCapabilities {
            can_read: true,
            can_set: true,
            can_watch: true,
        }
    }

    fn current_mode(&self) -> Result<Mode> {
        current_mode()
    }

    fn set_mode(&self, mode: DesiredMode) -> Result<()> {
        set_mode(mode)
    }
}

pub fn run_appearance_notification_loop(sender: Sender<()>) -> Result<()> {
    APPEARANCE_SENDER
        .set(sender)
        .map_err(|_| anyhow!("appearance notification loop already initialized"))?;

    unsafe {
        // Force Foundation to be linked/loaded before asking the Objective-C
        // runtime for Foundation classes by name.
        let _ = NSFoundationVersionNumber;

        let center_cls = objc_class("NSDistributedNotificationCenter")?;
        let string_cls = objc_class("NSString")?;
        let observer_cls = appearance_observer_class()?;
        let observer = class_createInstance(observer_cls, 0);
        if observer.is_null() {
            bail!("failed to create LumaAppearanceObserver instance");
        }

        let default_center: unsafe extern "C" fn(Class, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        let center = default_center(center_cls, objc_sel("defaultCenter")?);
        if center.is_null() {
            bail!("NSDistributedNotificationCenter.defaultCenter returned nil");
        }

        let notification_name = ns_string(string_cls, APPEARANCE_NOTIFICATION)?;
        let add_observer: unsafe extern "C" fn(Id, Sel, Id, Sel, Id, Id) =
            std::mem::transmute(objc_msgSend as *const ());
        add_observer(
            center,
            objc_sel("addObserver:selector:name:object:")?,
            observer,
            objc_sel("appearanceChanged:")?,
            notification_name,
            std::ptr::null_mut(),
        );

        CFRunLoopRun();
    }

    Ok(())
}

fn appearance_observer_class() -> Result<Class> {
    let class_name = CString::new("LumaAppearanceObserver")?;
    let existing = unsafe { objc_getClass(class_name.as_ptr()) };
    if !existing.is_null() {
        return Ok(existing);
    }

    let superclass = objc_class("NSObject")?;
    let cls = unsafe { objc_allocateClassPair(superclass, class_name.as_ptr(), 0) };
    if cls.is_null() {
        bail!("failed to allocate LumaAppearanceObserver class");
    }

    let types = CString::new("v@:@")?;
    let added = unsafe {
        class_addMethod(
            cls,
            objc_sel("appearanceChanged:")?,
            appearance_objc_callback as Imp,
            types.as_ptr(),
        )
    };
    if added == 0 {
        bail!("failed to add appearanceChanged: method");
    }
    unsafe { objc_registerClassPair(cls) };
    Ok(cls)
}

fn objc_class(name: &str) -> Result<Class> {
    let name = CString::new(name)?;
    let cls = unsafe { objc_getClass(name.as_ptr()) };
    if cls.is_null() {
        bail!("Objective-C class not found: {}", name.to_string_lossy());
    }
    Ok(cls)
}

fn objc_sel(name: &str) -> Result<Sel> {
    let name = CString::new(name)?;
    let sel = unsafe { sel_registerName(name.as_ptr()) };
    if sel.is_null() {
        bail!("Objective-C selector not found: {}", name.to_string_lossy());
    }
    Ok(sel)
}

fn ns_string(string_cls: Class, value: &str) -> Result<Id> {
    let value = CString::new(value)?;
    let string_with_utf8: unsafe extern "C" fn(Class, Sel, *const c_char) -> Id =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    let string = unsafe {
        string_with_utf8(
            string_cls,
            objc_sel("stringWithUTF8String:")?,
            value.as_ptr(),
        )
    };
    if string.is_null() {
        bail!("failed to create NSString");
    }
    Ok(string)
}

extern "C" fn appearance_objc_callback(_this: Id, _cmd: Sel, _notification: Id) {
    if let Some(sender) = APPEARANCE_SENDER.get() {
        let _ = sender.send(());
    }
}

pub fn current_mode() -> Result<Mode> {
    let status = Command::new("defaults")
        .args(["read", "-g", "AppleInterfaceStyle"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to run defaults")?;
    Ok(if status.success() {
        Mode::Dark
    } else {
        Mode::Light
    })
}

pub fn set_mode(mode: DesiredMode) -> Result<()> {
    let script = match mode {
        DesiredMode::Toggle => {
            r#"tell application "System Events"
  tell appearance preferences
    set dark mode to not dark mode
  end tell
end tell
"#
        }
        DesiredMode::Dark => {
            r#"tell application "System Events"
  tell appearance preferences
    set dark mode to true
  end tell
end tell
"#
        }
        DesiredMode::Light => {
            r#"tell application "System Events"
  tell appearance preferences
    set dark mode to false
  end tell
end tell
"#
        }
    };

    let mut child = Command::new("osascript")
        .stdin(Stdio::piped())
        .spawn()
        .context("failed to start osascript")?;
    child
        .stdin
        .take()
        .context("failed to open osascript stdin")?
        .write_all(script.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        bail!("osascript failed with status {status}");
    }
    Ok(())
}

pub fn preferences_dir() -> Result<PathBuf> {
    Ok(home_dir()?.join("Library/Preferences"))
}

pub fn launch_agent_file() -> Result<PathBuf> {
    Ok(home_dir()?.join(format!("Library/LaunchAgents/{LAUNCH_LABEL}.plist")))
}

pub fn write_launch_agent() -> Result<()> {
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{label}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{home}/.local/bin/lumactl</string>
		<string>watch</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<true/>
	<key>StandardErrorPath</key>
	<string>/tmp/{label}.err</string>
	<key>StandardOutPath</key>
	<string>/tmp/{label}.out</string>
</dict>
</plist>
"#,
        label = LAUNCH_LABEL,
        home = home_dir()?.display(),
    );
    write_if_changed(&launch_agent_file()?, &plist)
}

pub fn reload_launch_agent() -> Result<()> {
    let uid = current_uid()?;
    let plist = launch_agent_file()?;
    let domain = format!("gui/{uid}");
    let service = format!("{domain}/{LAUNCH_LABEL}");

    unload_launch_agent()?;

    command_ok(
        Command::new("launchctl")
            .arg("bootstrap")
            .arg(&domain)
            .arg(&plist),
        "launchctl bootstrap",
    )?;
    let _ = Command::new("launchctl")
        .args(["kickstart", "-k", &service])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(())
}

pub fn unload_launch_agent() -> Result<()> {
    let uid = current_uid()?;
    let domain = format!("gui/{uid}");
    let plist = launch_agent_file()?;
    let _ = Command::new("launchctl")
        .args(["bootout", &domain, &plist.display().to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    Ok(())
}

pub fn current_uid() -> Result<String> {
    let output = Command::new("id").arg("-u").output()?;
    if !output.status.success() {
        bail!("id -u failed");
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

pub fn install_binary(current_exe: &Path) -> Result<()> {
    let bin_dir = local_bin_dir()?;
    fs::create_dir_all(&bin_dir)?;
    let dest = bin_dir.join("lumactl");
    if !same_file_best_effort(current_exe, &dest) {
        fs::copy(current_exe, &dest).with_context(|| {
            format!(
                "failed to copy {} to {}",
                current_exe.display(),
                dest.display()
            )
        })?;
        set_executable(&dest)?;
    }
    Ok(())
}

fn same_file_best_effort(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn set_executable(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
    }
    Ok(())
}

fn command_ok(command: &mut Command, label: &str) -> Result<()> {
    let status = command
        .status()
        .with_context(|| format!("failed to run {label}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("{label} failed with status {status}"))
    }
}
