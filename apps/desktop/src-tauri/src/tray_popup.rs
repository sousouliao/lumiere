use serde_json::{Value, json};
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

#[derive(Default)]
pub struct PopupState {
    ready: AtomicBool,
    requested: AtomicBool,
    snapshot: Mutex<Value>,
}

fn snapshot(surface: &Value, settings: &Value, busy: bool) -> Value {
    let item = |mode: &str| {
        let shortcut = &settings["captureShortcuts"][mode];
        let label = match mode {
            "region" => "Capture Region",
            "display" => "Capture Display",
            _ => "Capture",
        };
        json!({
            "label": label,
            "shortcut":if shortcut["status"] == "registered" {
                shortcut["accelerator"].as_str().unwrap_or_default()
                    .replace("Control", "Ctrl").replace("Command", "Super")
            } else { String::new() },
            "enabled":surface["hostAvailable"] == true && !busy
                && surface["captureModes"].as_array().is_some_and(|modes| modes.iter().any(|value| value == mode))
        })
    };
    json!({"region":item("region"), "display":item("display")})
}

pub fn update(app: &AppHandle, surface: &Value, settings: &Value, busy: bool) {
    let Some(state) = app.try_state::<PopupState>() else {
        return;
    };
    let mut snapshot = snapshot(surface, settings, busy);
    let mut previous = state.snapshot.lock().expect("Tray state poisoned");
    if previous["region"] == snapshot["region"] && previous["display"] == snapshot["display"] {
        return;
    }
    snapshot["revision"] = json!(previous["revision"].as_u64().unwrap_or(0) + 1);
    *previous = snapshot.clone();
    drop(previous);
    if let Some(window) = app.get_webview_window("tray-menu") {
        let _ = window.emit("tray-menu-changed", &snapshot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_capture_requires_available_mode_and_idle_host() {
        let surface = json!({"hostAvailable":true,"captureModes":["region"]});
        let idle = snapshot(&surface, &Value::Null, false);
        assert_eq!(idle["region"]["enabled"], true);
        assert_eq!(idle["display"]["enabled"], false);
        assert_eq!(
            snapshot(&surface, &Value::Null, true)["region"]["enabled"],
            false
        );
        assert_eq!(
            snapshot(
                &json!({"hostAvailable":false,"captureModes":["region"]}),
                &Value::Null,
                false
            )["region"]["enabled"],
            false
        );
    }

    #[test]
    fn menu_shows_only_registered_shortcuts() {
        let settings = json!({"captureShortcuts":{
            "region":{"status":"registered","accelerator":"Control+Shift+4"},
            "display":{"status":"conflict","accelerator":"Control+Shift+5"}
        }});
        let menu = snapshot(&Value::Null, &settings, false);
        assert_eq!(menu["region"]["shortcut"], "Ctrl+Shift+4");
        assert_eq!(menu["display"]["shortcut"], "");
    }
}

pub fn prepare(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window("tray-menu") {
        return Ok(window);
    }
    app.state::<PopupState>()
        .ready
        .store(false, Ordering::Release);
    let window = WebviewWindowBuilder::new(
        app,
        "tray-menu",
        WebviewUrl::App("index.html?view=tray-menu".into()),
    )
    .title("Lumiere menu")
    .inner_size(278.0, 184.0)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .resizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .focused(false)
    .visible(false)
    .background_color(tauri::window::Color(0, 0, 0, 0))
    .on_navigation(|url| {
        matches!(url.scheme(), "tauri" | "http")
            && matches!(
                url.host_str(),
                Some("localhost" | "tauri.localhost" | "127.0.0.1")
            )
    })
    .build()?;
    if let Err(error) = crate::tray_icon::attach(&window) {
        eprintln!(
            "{}",
            json!({"event":"tray-theme-listener-error", "error":error.to_string()})
        );
    }
    Ok(window)
}

fn present(window: &WebviewWindow) -> tauri::Result<()> {
    window.emit("tray-menu-opened", ())?;
    window.show()?;
    window.set_focus()
}

pub fn dismiss(window: &WebviewWindow) -> tauri::Result<()> {
    window
        .app_handle()
        .state::<PopupState>()
        .requested
        .store(false, Ordering::Release);
    window.hide()
}

#[tauri::command]
pub fn tray_menu_ready(window: WebviewWindow) -> Result<(), String> {
    if window.label() != "tray-menu" {
        return Err("Unexpected menu window".into());
    }
    let app = window.app_handle();
    let state = app.state::<PopupState>();
    state.ready.store(true, Ordering::Release);
    if state.requested.load(Ordering::Acquire) {
        present(&window).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn show(
    app: &AppHandle,
    position: tauri::PhysicalPosition<f64>,
    icon: tauri::Rect,
) -> tauri::Result<()> {
    if let Some(window) = app.get_webview_window("tray-menu")
        && window.is_visible()?
    {
        return dismiss(&window);
    }
    let window = prepare(app)?;
    let placed = (|| {
        if let Some(monitor) = window.monitor_from_point(position.x, position.y)? {
            let work = monitor.work_area();
            let scale = monitor.scale_factor();
            let width = (278.0 * scale).round();
            let height = (184.0 * scale).round();
            let anchor = icon.position.to_physical::<f64>(scale);
            // Align the visible menu with the tray hit area, independent of cursor position.
            let x = (anchor.x - 8.0 * scale).clamp(
                work.position.x as f64,
                (work.position.x as f64 + work.size.width as f64 - width)
                    .max(work.position.x as f64),
            );
            // Leave 4 DIP between the visible menu and taskbar. The remaining
            // transparent shadow padding may extend beyond the work area.
            let bottom = anchor
                .y
                .min(work.position.y as f64 + work.size.height as f64);
            let y = (bottom - height + 4.0 * scale).clamp(
                work.position.y as f64,
                (work.position.y as f64 + work.size.height as f64 - height + 4.0 * scale)
                    .max(work.position.y as f64),
            );
            window.set_position(tauri::PhysicalPosition::new(x as i32, y as i32))?;
            window.set_size(tauri::LogicalSize::new(278.0, 184.0))?;
        }
        Ok(())
    })();
    if placed.is_err() {
        let _ = dismiss(&window);
        return placed;
    }
    let state = app.state::<PopupState>();
    state.requested.store(true, Ordering::Release);
    if state.ready.load(Ordering::Acquire) {
        present(&window)?;
    }
    Ok(())
}

#[tauri::command(async)]
pub fn get_tray_menu_snapshot(app: AppHandle) -> Result<Value, String> {
    let state = app.state::<PopupState>();
    let snapshot = state
        .snapshot
        .lock()
        .map_err(|_| "Tray state poisoned")?
        .clone();
    Ok(if snapshot.is_null() {
        json!({
            "revision":0,
            "region":{"label":"Capture Region", "shortcut":"", "enabled":false},
            "display":{"label":"Capture Display", "shortcut":"", "enabled":false}
        })
    } else {
        snapshot
    })
}

#[tauri::command(async)]
pub fn tray_menu_action(window: WebviewWindow, action: String) -> Result<(), String> {
    if window.label() != "tray-menu"
        || !matches!(
            action.as_str(),
            "region" | "display" | "open" | "settings" | "quit"
        )
    {
        return Err("Unknown tray action".into());
    }
    let app = window.app_handle().clone();
    if matches!(action.as_str(), "region" | "display")
        && get_tray_menu_snapshot(app.clone())?[&action]["enabled"] != true
    {
        return Err("Capture is currently unavailable".into());
    }
    eprintln!(
        "{}",
        json!({"event":"tray-action-requested", "action":action})
    );
    // Accept the action before dismissing its originating WebView. Run window
    // creation outside the IPC/main-thread callback, as required by WebView2.
    tauri::async_runtime::spawn(async move {
        if let Err(error) = dismiss(&window) {
            eprintln!(
                "{}",
                json!({"event":"tray-menu-hide-error", "error":error.to_string()})
            );
        }
        crate::handle_tray_action(&app, &action);
    });
    Ok(())
}
