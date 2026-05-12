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
    Platform, home_dir, local_bin_dir, validate_relative_path, write_if_changed,
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
        let relative = validate_relative_path(relative)?;
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
        let relative = validate_relative_path(relative)?;
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

    // SAFETY: This block bridges to Objective-C/Foundation APIs. Class and
    // selector lookups are checked for null before use, Objective-C strings are
    // built from `CString`s that live for each message send, and the observer is
    // intentionally kept alive for the lifetime of the run loop.
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

        // SAFETY: `defaultCenter` is a class method with Objective-C ABI
        // equivalent to `(Class, Sel) -> id` for this message send.
        let default_center: unsafe extern "C" fn(Class, Sel) -> Id =
            std::mem::transmute(objc_msgSend as *const ());
        let center = default_center(center_cls, objc_sel("defaultCenter")?);
        if center.is_null() {
            bail!("NSDistributedNotificationCenter.defaultCenter returned nil");
        }

        let notification_name = ns_string(string_cls, APPEARANCE_NOTIFICATION)?;
        // SAFETY: `addObserver:selector:name:object:` has the Objective-C ABI
        // shape `(id, SEL, id, SEL, id, id) -> void`; the return value is ignored.
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
    // SAFETY: `class_name` is a NUL-terminated CString; null return is checked.
    let existing = unsafe { objc_getClass(class_name.as_ptr()) };
    if !existing.is_null() {
        return Ok(existing);
    }

    let superclass = objc_class("NSObject")?;
    // SAFETY: `superclass` is a valid Objective-C class from `objc_class`, and
    // `class_name` is a stable NUL-terminated string for the duration of call.
    let cls = unsafe { objc_allocateClassPair(superclass, class_name.as_ptr(), 0) };
    if cls.is_null() {
        bail!("failed to allocate LumaAppearanceObserver class");
    }

    let types = CString::new("v@:@")?;
    // SAFETY: `cls` is newly allocated, selector/type encoding match
    // `appearance_objc_callback`, and all pointers are valid for this call.
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
    // SAFETY: `cls` was allocated by `objc_allocateClassPair` and not yet registered.
    unsafe { objc_registerClassPair(cls) };
    Ok(cls)
}

fn objc_class(name: &str) -> Result<Class> {
    let name = CString::new(name)?;
    // SAFETY: `name` is a NUL-terminated CString; null return is checked.
    let cls = unsafe { objc_getClass(name.as_ptr()) };
    if cls.is_null() {
        bail!("Objective-C class not found: {}", name.to_string_lossy());
    }
    Ok(cls)
}

fn objc_sel(name: &str) -> Result<Sel> {
    let name = CString::new(name)?;
    // SAFETY: `name` is a NUL-terminated CString; null return is checked.
    let sel = unsafe { sel_registerName(name.as_ptr()) };
    if sel.is_null() {
        bail!("Objective-C selector not found: {}", name.to_string_lossy());
    }
    Ok(sel)
}

fn ns_string(string_cls: Class, value: &str) -> Result<Id> {
    let value = CString::new(value)?;
    // SAFETY: `stringWithUTF8String:` has ABI shape
    // `(Class, Sel, *const c_char) -> id`; `value` stays alive for the send.
    let string_with_utf8: unsafe extern "C" fn(Class, Sel, *const c_char) -> Id =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    // SAFETY: `string_cls` is `NSString`, selector and C string pointer are valid
    // for this Objective-C message send, and null return is checked below.
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
    let label = xml_escape(LAUNCH_LABEL);
    let home = xml_escape(&home_dir()?.display().to_string());
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
        label = label,
        home = home,
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
        replace_binary(current_exe, &dest)?;
    }
    Ok(())
}

fn replace_binary(current_exe: &Path, dest: &Path) -> Result<()> {
    let tmp = dest.with_file_name(format!(
        ".lumactl.{}.tmp",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default()
    ));

    fs::copy(current_exe, &tmp).with_context(|| {
        format!(
            "failed to copy {} to {}",
            current_exe.display(),
            tmp.display()
        )
    })?;
    set_executable(&tmp)?;
    if let Err(err) = fs::rename(&tmp, dest) {
        let _ = fs::remove_file(&tmp);
        return Err(err).with_context(|| format!("failed to replace {}", dest.display()));
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

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn macos_paths_reject_relative_escape_components_before_joining() {
        let macos = MacOs;

        assert!(
            macos
                .app_config_files(AppId::Nvim, Path::new("../outside"))
                .is_err()
        );
        assert!(
            macos
                .app_cache_file(AppId::Luma, Path::new("/tmp/outside"))
                .is_err()
        );
    }

    #[test]
    fn xml_escape_escapes_plist_special_characters() {
        assert_eq!(
            xml_escape("/Users/a&b/<luma>\"'"),
            "/Users/a&amp;b/&lt;luma&gt;&quot;&apos;"
        );
    }
}
