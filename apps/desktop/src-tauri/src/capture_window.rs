//! Hide on the window thread, then wait for composition before requesting a frame.
use tauri::WebviewWindow;
use windows::Win32::{
    Graphics::Dwm::{DWMWA_TRANSITIONS_FORCEDISABLED, DwmFlush, DwmSetWindowAttribute},
    UI::WindowsAndMessaging::{IsWindowVisible, SW_HIDE, ShowWindow},
};
use windows::core::BOOL;

pub async fn hide_for_capture(window: &WebviewWindow) -> Result<(), String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let owned = window.clone();
    window
        .run_on_main_thread(move || {
            let result = hide(&owned);
            let _ = tx.send(result);
        })
        .map_err(|error| error.to_string())?;
    rx.await.map_err(|error| error.to_string())?
}

fn hide(window: &WebviewWindow) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    let enabled = BOOL(0);
    let disabled = BOOL(1);
    // SAFETY: Called on this live window's owning thread. Attribute pointers refer
    // to stack BOOLs with the exact API size; no HWND or pointer escapes the call.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            (&disabled as *const BOOL).cast(),
            size_of::<BOOL>() as u32,
        )
        .map_err(|error| error.to_string())?;
        // ShowWindow's return value is the previous visibility, not a success flag.
        let _ = ShowWindow(hwnd, SW_HIDE);
        let hidden = !IsWindowVisible(hwnd).as_bool();
        let composed = DwmFlush().map_err(|error| error.to_string());
        // Restore the default transition policy of our own main window.
        let restored = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            (&enabled as *const BOOL).cast(),
            size_of::<BOOL>() as u32,
        )
        .map_err(|error| error.to_string());
        composed?;
        restored?;
        if !hidden {
            return Err("Main window remained visible".into());
        }
    }
    Ok(())
}
