use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Manager, WebviewWindow, image::Image};
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::Registry::*,
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{WM_NCDESTROY, WM_SETTINGCHANGE},
        },
    },
    core::w,
};

pub struct ThemeState(pub AtomicBool);

pub fn system_is_dark() -> Option<bool> {
    let mut light = 0_u32;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: Read only one DWORD into correctly sized storage using the predefined HKCU handle.
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut light as *mut u32).cast()),
            Some(&mut size),
        )
        .ok()
        .ok()?;
    }
    Some(light == 0)
}

pub fn image(dark: bool) -> tauri::Result<Image<'static>> {
    let source = Image::from_bytes(include_bytes!("../../resources/icons/windows/tray.png"))?;
    let mut rgba = source.rgba().to_vec();
    let foreground = if dark { [255, 255, 255] } else { [31, 29, 27] };
    for pixel in rgba.chunks_exact_mut(4) {
        pixel[..3].copy_from_slice(&foreground);
    }
    Ok(Image::new_owned(rgba, source.width(), source.height()))
}

fn refresh(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if app
            .state::<crate::Controller>()
            .quitting
            .load(Ordering::Acquire)
        {
            return;
        }
        // Serialize theme updates on the icon's owning thread; no native work
        // runs inside the settings-notification callback.
        let dispatch = app.clone();
        let _ = app.run_on_main_thread(move || {
            // Read after dispatch so rapid successive notifications cannot apply
            // a stale theme captured by an earlier background task.
            let Some(next) = system_is_dark() else {
                return;
            };
            let theme = dispatch.state::<ThemeState>();
            if theme.0.load(Ordering::Acquire) == next {
                return;
            }
            let Some(tray) = dispatch.tray_by_id("lumiere") else {
                return;
            };
            match image(next).and_then(|icon| tray.set_icon(Some(icon))) {
                Ok(()) => theme.0.store(next, Ordering::Release),
                Err(error) => eprintln!(
                    "{}",
                    serde_json::json!({"event":"tray-icon-theme-error", "error":error.to_string()})
                ),
            }
        });
    });
}

const THEME_SUBCLASS: usize = 0x4c554d49;

unsafe extern "system" fn theme_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: The subclass owns its boxed AppHandle until WM_NCDESTROY;
    // registration/removal and this callback all run on the HWND's owning thread.
    unsafe {
        if message == WM_SETTINGCHANGE {
            refresh((&*(data as *const AppHandle)).clone());
        } else if message == WM_NCDESTROY {
            let _ = RemoveWindowSubclass(window, Some(theme_proc), id);
            drop(Box::from_raw(data as *mut AppHandle));
        }
        DefSubclassProc(window, message, wparam, lparam)
    }
}

pub fn attach(window: &WebviewWindow) -> tauri::Result<()> {
    let hwnd = window.hwnd()?;
    let app = window.app_handle().clone();
    let data = Box::into_raw(Box::new(app.clone()));
    // SAFETY: prepare calls this on the window's owning thread. Ownership passes
    // to the subclass on success and is released on failure or WM_NCDESTROY.
    unsafe {
        if !SetWindowSubclass(hwnd, Some(theme_proc), THEME_SUBCLASS, data as usize).as_bool() {
            drop(Box::from_raw(data));
            return Err(std::io::Error::other("Could not attach Windows theme listener").into());
        }
    }
    // Cover a settings change between the startup read and listener installation.
    refresh(app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_variants_preserve_source_silhouette_and_dimensions() {
        let source =
            Image::from_bytes(include_bytes!("../../resources/icons/windows/tray.png")).unwrap();
        for (dark, rgb) in [(true, [255, 255, 255]), (false, [31, 29, 27])] {
            let variant = image(dark).unwrap();
            assert_eq!(
                (variant.width(), variant.height()),
                (source.width(), source.height())
            );
            for (original, tinted) in source
                .rgba()
                .chunks_exact(4)
                .zip(variant.rgba().chunks_exact(4))
            {
                assert_eq!(tinted[3], original[3]);
                assert_eq!(&tinted[..3], &rgb);
            }
        }
    }
}
