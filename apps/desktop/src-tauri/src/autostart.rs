//! Current-user login registration. Windows owns the state; settings.json does not.
use serde::Serialize;
use std::{os::windows::ffi::OsStrExt, path::Path};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use windows::{
    Win32::{Foundation::ERROR_FILE_NOT_FOUND, System::Registry::*},
    core::{PCWSTR, w},
};

const RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const APPROVED: PCWSTR =
    w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run");
#[cfg(not(test))]
const NAME: PCWSTR = w!("Lumiere");
// GUI fixtures exercise the real commands without altering the installed app's entry.
#[cfg(test)]
const NAME: PCWSTR = w!("Lumiere-test-autostart");
pub const ARGUMENT: &str = "--autostart";

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum Snapshot {
    Enabled,
    Disabled,
    Blocked,
    Unavailable { message: String },
}

struct Key(HKEY);
impl Drop for Key {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns an opened HKCU key and closes it exactly once.
        unsafe {
            let _ = RegCloseKey(self.0);
        }
    }
}

fn command(path: &Path) -> Result<Vec<u16>, String> {
    let mut value = vec![b'"' as u16];
    value.extend(path.as_os_str().encode_wide());
    value.extend(format!("\" {ARGUMENT}").encode_utf16());
    if !path.is_absolute() || value.len() > 260 {
        return Err("The application path cannot be registered for launch at login.".into());
    }
    value.push(0);
    Ok(value)
}

fn read(key: PCWSTR, name: PCWSTR, flags: REG_ROUTINE_FLAGS) -> Result<Option<Vec<u8>>, String> {
    let mut size = 0;
    // SAFETY: predefined HKCU root, static/owned terminated names and writable size.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            flags,
            None,
            None,
            Some(&mut size),
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    status.ok().map_err(|error| error.to_string())?;
    let mut bytes = vec![0; size as usize];
    // SAFETY: the buffer has the queried byte capacity and remains owned throughout the call.
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            name,
            flags,
            None,
            Some(bytes.as_mut_ptr().cast()),
            Some(&mut size),
        )
        .ok()
        .map_err(|error| error.to_string())?;
    }
    bytes.truncate(size as usize);
    Ok(Some(bytes))
}

fn approval(bytes: &[u8]) -> Result<bool, String> {
    // StartupApproved is not a documented Windows API. Accept only known records;
    // an unfamiliar representation must never be reported as enabled.
    if bytes.len() == 12 {
        match u32::from_le_bytes(bytes[..4].try_into().unwrap()) {
            2 | 6 => return Ok(true),
            3 | 7 => return Ok(false),
            _ => {}
        }
    }
    Err("Unable to read Windows startup status. Check Startup Apps in Windows Settings.".into())
}

fn registered_snapshot(name: PCWSTR, expected: &[u16]) -> Result<Snapshot, String> {
    let Some(value) = read(RUN, name, RRF_RT_REG_SZ)? else {
        return Ok(Snapshot::Disabled);
    };
    let expected: Vec<u8> = expected
        .iter()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    if value != expected {
        // A previous location is not registration for this executable. Only an
        // explicit enable action replaces it; opening the app never repairs it.
        return Ok(Snapshot::Disabled);
    }
    match read(APPROVED, name, RRF_RT_REG_BINARY)? {
        Some(bytes) if !approval(&bytes)? => Ok(Snapshot::Blocked),
        _ => Ok(Snapshot::Enabled),
    }
}

fn write_registration(name: PCWSTR, value: Option<&[u16]>) -> Result<(), String> {
    let mut handle = HKEY::default();
    // SAFETY: only HKCU Run is opened/created; output storage is valid. No security
    // descriptor is supplied, so the key inherits the current user's permissions.
    let status = unsafe {
        if value.is_some() {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                RUN,
                None,
                PCWSTR::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut handle,
                None,
            )
        } else {
            RegOpenKeyExW(HKEY_CURRENT_USER, RUN, None, KEY_SET_VALUE, &mut handle)
        }
    };
    if value.is_none() && status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    status.ok().map_err(|error| error.to_string())?;
    let key = Key(handle);
    let bytes = value.map(|value| {
        value
            .iter()
            .flat_map(|unit| unit.to_le_bytes())
            .collect::<Vec<_>>()
    });
    // SAFETY: the opened key is owned, the name is terminated and the REG_SZ byte
    // buffer includes its UTF-16 terminator. Deletion targets only this named value.
    let status = unsafe {
        match bytes {
            Some(bytes) => RegSetValueExW(key.0, name, None, REG_SZ, Some(&bytes)),
            None => RegDeleteValueW(key.0, name),
        }
    };
    if value.is_none() && status == ERROR_FILE_NOT_FOUND {
        return Ok(());
    }
    status.ok().map_err(|error| error.to_string())
}

fn unavailable(error: String) -> Snapshot {
    eprintln!(
        "{}",
        serde_json::json!({"event":"autostart-read-error", "error":error})
    );
    Snapshot::Unavailable {
        message: "Unable to read launch at login. Try again or check Windows Startup Apps."
            .into(),
    }
}

#[tauri::command(async)]
pub fn get_autostart_snapshot() -> Snapshot {
    if cfg!(debug_assertions) {
        return Snapshot::Unavailable {
            message: "Launch at login is available in installed release builds.".into(),
        };
    }
    std::env::current_exe()
        .map_err(|error| error.to_string())
        .and_then(|path| command(&path))
        .and_then(|expected| registered_snapshot(NAME, &expected))
        .unwrap_or_else(unavailable)
}

#[tauri::command(async)]
pub fn set_autostart_enabled(enabled: bool) -> Result<Snapshot, String> {
    if cfg!(debug_assertions) {
        return Err("Launch at login is unavailable in development builds.".into());
    }
    let result = (|| {
        let path = std::env::current_exe().map_err(|error| error.to_string())?;
        let expected = command(&path)?;
        write_registration(NAME, enabled.then_some(expected.as_slice()))?;
        registered_snapshot(NAME, &expected)
    })();
    result.map_err(|error: String| {
        eprintln!(
            "{}",
            serde_json::json!({"event":"autostart-write-error", "enabled":enabled, "error":error})
        );
        "Unable to change launch at login. Try again or check Windows Startup Apps.".into()
    })
}

#[tauri::command(async)]
pub fn open_startup_settings(app: AppHandle) -> Result<(), String> {
    app.opener()
        .open_url("ms-settings:startupapps", None::<&str>)
        .map_err(|error| error.to_string())
}

pub fn is_autostart(arguments: &[String]) -> bool {
    arguments
        .iter()
        .skip(1)
        .any(|argument| argument == ARGUMENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autostart_quotes_unicode_path_and_limits_command() {
        let value = command(Path::new(r"C:\工具 folder\Lumiere.exe")).unwrap();
        assert_eq!(
            String::from_utf16(&value[..value.len() - 1]).unwrap(),
            "\"C:\\工具 folder\\Lumiere.exe\" --autostart"
        );
        assert!(command(Path::new("Lumiere.exe")).is_err());
        assert!(command(Path::new(&format!("C:\\{}\\Lumiere.exe", "x".repeat(260)))).is_err());
    }

    #[test]
    fn autostart_known_and_unknown_approval_records() {
        for (state, expected) in [(2_u32, true), (3, false), (6, true), (7, false)] {
            let mut bytes = [0; 12];
            bytes[..4].copy_from_slice(&state.to_le_bytes());
            assert_eq!(approval(&bytes), Ok(expected));
        }
        assert!(approval(&[0; 12]).is_err());
        assert!(approval(&[2; 8]).is_err());
    }

    #[test]
    fn autostart_only_exact_startup_argument_suppresses_activation() {
        assert!(is_autostart(&["Lumiere.exe".into(), ARGUMENT.into()]));
        assert!(!is_autostart(&["Lumiere.exe".into()]));
        assert!(!is_autostart(&[ARGUMENT.into()]));
        assert!(!is_autostart(&[
            "Lumiere.exe".into(),
            "--autostart-other".into()
        ]));
    }

    #[test]
    fn autostart_current_user_registration_roundtrip() {
        // Unique reserved fixture value, removed even on panic. Never touch Lumiere.
        let name: Vec<u16> = format!("Lumiere-test-autostart-{}", std::process::id())
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let name = PCWSTR(name.as_ptr());
        struct Cleanup(PCWSTR);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = write_registration(self.0, None);
            }
        }
        assert!(read(RUN, name, RRF_RT_REG_SZ).unwrap().is_none());
        let _cleanup = Cleanup(name);
        let expected = command(&std::env::current_exe().unwrap()).unwrap();
        assert_eq!(
            registered_snapshot(name, &expected).unwrap(),
            Snapshot::Disabled
        );
        write_registration(name, Some(&expected)).unwrap();
        assert_eq!(
            registered_snapshot(name, &expected).unwrap(),
            Snapshot::Enabled
        );
        write_registration(name, None).unwrap();
        write_registration(name, None).unwrap();
        assert_eq!(
            registered_snapshot(name, &expected).unwrap(),
            Snapshot::Disabled
        );
    }

    #[test]
    fn autostart_debug_commands_do_not_touch_registration() {
        if cfg!(debug_assertions) {
            assert!(matches!(
                get_autostart_snapshot(),
                Snapshot::Unavailable { .. }
            ));
            assert!(set_autostart_enabled(true).is_err());
            assert!(set_autostart_enabled(false).is_err());
        }
    }
}
