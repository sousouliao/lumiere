#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod capture_window;
mod controller;
mod host;
mod notification;
mod settings;
#[cfg(test)]
mod tests;
mod tray_icon;
mod tray_popup;
mod updater;

use controller::Controller;
use lumiere_capture_contract::{CaptureMode, Delivery};
use serde::Deserialize;
use serde_json::Value;
use settings::AfterCapture;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_dialog::DialogExt;

fn show_window(app: &AppHandle, settings: bool) -> tauri::Result<()> {
    if app
        .state::<Controller>()
        .quitting
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return Ok(());
    }
    if let Some(window) = app.get_webview_window("main") {
        window.unminimize()?;
        window.show()?;
        window.set_focus()?;
        window.emit(
            if settings {
                "show-settings-requested"
            } else {
                "show-capture-requested"
            },
            (),
        )?;
    } else {
        let url = if settings {
            "index.html?view=settings"
        } else {
            "index.html"
        };
        let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::App(url.into()))
            .title("Lumiere")
            .inner_size(480., 370.)
            .min_inner_size(440., 340.)
            .decorations(false)
            .visible(false)
            .background_color(tauri::window::Color(31, 29, 27, 255))
            .on_navigation(|url| {
                matches!(url.scheme(), "tauri" | "http")
                    && matches!(
                        url.host_str(),
                        Some("localhost" | "tauri.localhost" | "127.0.0.1")
                    )
            })
            .build()?;
        // Electron's Windows overlay frame leaves one extra physical client pixel
        // vertically (720x556 at 150%). Preserve its React viewport rounding.
        let mut size = window.inner_size()?;
        size.height += 1;
        window.set_size(size)?;
    }
    Ok(())
}

#[tauri::command]
fn renderer_ready(window: tauri::WebviewWindow) -> Result<(), String> {
    window
        .show()
        .and_then(|()| window.set_focus())
        .map_err(|error| error.to_string())
}
#[tauri::command]
async fn get_capture_surface_snapshot(app: AppHandle) -> Value {
    app.state::<Controller>().surface().await
}
#[tauri::command]
async fn refresh_capture_surface(app: AppHandle) -> Value {
    app.state::<Controller>().refresh(&app).await
}
#[tauri::command(async)]
fn get_capture_activity(state: tauri::State<'_, Controller>) -> Value {
    state.activity()
}
#[tauri::command]
async fn capture_display(app: AppHandle) -> Value {
    app.state::<Controller>()
        .capture(&app, CaptureMode::Display)
        .await
}
#[tauri::command]
async fn capture_region(app: AppHandle) -> Value {
    app.state::<Controller>()
        .capture(&app, CaptureMode::Region)
        .await
}
#[tauri::command(async)]
fn get_settings_snapshot(state: tauri::State<'_, Controller>) -> Value {
    state.settings_snapshot()
}
#[tauri::command]
async fn choose_save_directory(app: AppHandle) -> Result<Value, String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let mut dialog = app.dialog().file().set_title("Choose save folder");
    if let Some(window) = app.get_webview_window("main") {
        dialog = dialog.set_parent(&window);
    }
    dialog.pick_folder(move |folder| {
        let _ = tx.send(folder);
    });
    let folder = rx.await.map_err(|error| error.to_string())?;
    if let Some(folder) = folder {
        let path = folder.into_path().map_err(|error| error.to_string())?;
        let snapshot = app
            .state::<Controller>()
            .update_settings(&app, |settings| settings.save_directory = Some(path))?;
        app.state::<Controller>().refresh(&app).await;
        Ok(snapshot)
    } else {
        Ok(app.state::<Controller>().settings_snapshot())
    }
}
#[tauri::command]
async fn set_output_delivery(app: AppHandle, delivery: Delivery) -> Result<Value, String> {
    let snapshot = app
        .state::<Controller>()
        .update_settings(&app, |settings| settings.output_delivery = delivery)?;
    app.state::<Controller>().refresh(&app).await;
    Ok(snapshot)
}
#[tauri::command(async)]
fn set_after_capture_behavior(app: AppHandle, behavior: AfterCapture) -> Result<Value, String> {
    app.state::<Controller>()
        .update_settings(&app, |settings| settings.after_capture_behavior = behavior)
}
#[tauri::command(async)]
fn set_hide_main_window_during_capture(app: AppHandle, enabled: bool) -> Result<Value, String> {
    app.state::<Controller>().update_settings(&app, |settings| {
        settings.hide_main_window_during_capture = enabled
    })
}
#[tauri::command]
async fn set_hdr_status_reminders(app: AppHandle, enabled: bool) -> Result<Value, String> {
    let snapshot = app
        .state::<Controller>()
        .update_settings(&app, |settings| settings.hdr_status_reminders = enabled)?;
    app.state::<Controller>().refresh(&app).await;
    Ok(snapshot)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShortcutUpdate {
    mode: CaptureMode,
    accelerator: Option<String>,
}
#[tauri::command(async)]
fn set_capture_shortcut(app: AppHandle, update: ShortcutUpdate) -> Value {
    app.state::<Controller>()
        .set_shortcut(&app, update.mode, update.accelerator)
}
#[tauri::command(async)]
fn set_shortcut_recording(app: AppHandle, recording: bool) {
    app.state::<Controller>().set_recording(&app, recording);
}
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
enum RecoveryAction {
    CaptureAgain,
    OpenSettings,
    ChooseFolder,
    Refresh,
}
#[tauri::command]
async fn recover_capture(app: AppHandle, id: u64, action: RecoveryAction) -> Result<(), String> {
    let mode = app
        .state::<Controller>()
        .completion_mode(id)
        .ok_or("Capture recovery is no longer current")?;
    match action {
        RecoveryAction::CaptureAgain => {
            app.state::<Controller>().capture(&app, mode).await;
        }
        RecoveryAction::OpenSettings => {
            show_window(&app, true).map_err(|error| error.to_string())?
        }
        RecoveryAction::ChooseFolder => {
            choose_save_directory(app).await?;
        }
        RecoveryAction::Refresh => {
            app.state::<Controller>().refresh(&app).await;
        }
    }
    Ok(())
}
#[tauri::command(async)]
fn get_update_snapshot(app: AppHandle) -> Value {
    app.state::<updater::Updates>().snapshot(&app)
}
#[tauri::command]
async fn check_for_updates(app: AppHandle) -> Value {
    app.state::<updater::Updates>().check(&app).await
}
#[tauri::command]
async fn download_update(app: AppHandle) -> Result<Value, String> {
    app.state::<updater::Updates>().download(&app).await
}
#[tauri::command]
async fn install_update(app: AppHandle) -> Result<Value, String> {
    app.state::<updater::Updates>().install(&app).await
}
fn host_path() -> Result<PathBuf, Box<dyn std::error::Error>> {
    // Launch failures belong to the recoverable capture surface, not shell startup.
    Ok(std::env::current_exe()?.with_file_name("lumiere-windows-host.exe"))
}

fn quit(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        app.state::<Controller>().shutdown().await;
        app.exit(0);
    });
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let dark = tray_icon::system_is_dark().unwrap_or(true);
    TrayIconBuilder::with_id("lumiere")
        .icon(tray_icon::image(dark)?)
        .tooltip("Lumiere")
        .show_menu_on_left_click(false)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Right, button_state: MouseButtonState::Up, position, rect, .. } = event {
                let app = tray.app_handle().clone();
                // Schedule creation after the tray event callback returns.
                tauri::async_runtime::spawn(async move {
                    let dispatch = app.clone();
                    let _ = app.run_on_main_thread(move || {
                        if let Err(error) = tray_popup::show(&dispatch, position, rect) {
                            eprintln!("{}", serde_json::json!({"event":"tray-menu-error", "error":error.to_string()}));
                        }
                    });
                });
            }
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                let _ = show_window(tray.app_handle(), false);
            }
        })
        .build(app)?;
    app.manage(tray_icon::ThemeState(std::sync::atomic::AtomicBool::new(
        dark,
    )));
    app.manage(tray_popup::PopupState::default());
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let dispatch = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Err(error) = tray_popup::prepare(&dispatch) {
                eprintln!("{}", serde_json::json!({"event":"tray-menu-preload-error", "error":error.to_string()}));
            }
        });
    });
    Ok(())
}

fn handle_tray_action(app: &AppHandle, action: &str) {
    match action {
        "open" => {
            if let Err(error) = show_window(app, false) {
                eprintln!(
                    "{}",
                    serde_json::json!({"event":"tray-action-error", "action":action, "error":error.to_string()})
                );
            }
        }
        "settings" => {
            if let Err(error) = show_window(app, true) {
                eprintln!(
                    "{}",
                    serde_json::json!({"event":"tray-action-error", "action":action, "error":error.to_string()})
                );
            }
        }
        "quit" => quit(app),
        "region" | "display" => {
            let mode = if action == "region" {
                CaptureMode::Region
            } else {
                CaptureMode::Display
            };
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                app.state::<Controller>().capture(&app, mode).await;
            });
        }
        _ => {}
    }
}

fn update_tray(app: &AppHandle, surface: &Value, settings: &Value, busy: bool) {
    tray_popup::update(app, surface, settings, busy);
}

fn cursor_target() -> Option<usize> {
    use windows::Win32::{
        Foundation::POINT,
        Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromPoint},
        UI::WindowsAndMessaging::GetCursorPos,
    };
    let mut point = POINT::default();
    // SAFETY: writable POINT storage; the returned monitor is an opaque identifier
    // used only to invalidate capabilities, never to acquire or convert pixels.
    unsafe {
        GetCursorPos(&mut point).ok()?;
        Some(MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST).0 as usize)
    }
}

struct Paths {
    host: PathBuf,
    settings: PathBuf,
}

fn builder(paths: Paths) -> tauri::Builder<tauri::Wry> {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, arguments, _| {
            activate_second_instance(app, &arguments);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_opener::Builder::new()
                .open_js_links_on_click(false)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            renderer_ready,
            tray_popup::get_tray_menu_snapshot,
            tray_popup::tray_menu_action,
            tray_popup::tray_menu_ready,
            get_capture_surface_snapshot,
            refresh_capture_surface,
            get_capture_activity,
            capture_display,
            capture_region,
            get_settings_snapshot,
            autostart::get_autostart_snapshot,
            autostart::set_autostart_enabled,
            autostart::open_startup_settings,
            choose_save_directory,
            set_output_delivery,
            set_after_capture_behavior,
            set_hdr_status_reminders,
            set_hide_main_window_during_capture,
            set_capture_shortcut,
            set_shortcut_recording,
            recover_capture,
            get_update_snapshot,
            check_for_updates,
            download_update,
            install_update
        ])
        .setup(move |app| {
            app.manage(Controller::new(
                paths.host,
                paths.settings,
                app.path().picture_dir()?.join("Lumiere"),
            ));
            app.manage(updater::Updates::new());
            updater::schedule(app.handle());
            app.state::<Controller>().initialize_shortcuts(app.handle());
            setup_tray(app.handle())?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                handle.state::<Controller>().refresh(&handle).await;
                let mut target = None;
                let mut timer = tokio::time::interval(std::time::Duration::from_millis(250));
                loop {
                    timer.tick().await;
                    if handle
                        .state::<Controller>()
                        .quitting
                        .load(std::sync::atomic::Ordering::Acquire)
                    {
                        break;
                    }
                    let visible = handle
                        .get_webview_window("main")
                        .is_some_and(|window| window.is_visible().unwrap_or(false));
                    if !visible {
                        target = None;
                        continue;
                    }
                    let next = cursor_target();
                    if next != target {
                        target = next;
                        handle.state::<Controller>().refresh(&handle).await;
                    }
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "tray-menu" {
                if matches!(event, tauri::WindowEvent::Focused(false))
                    && window.is_visible().unwrap_or(false)
                    && let Some(webview) = window.app_handle().get_webview_window("tray-menu")
                {
                    let _ = tray_popup::dismiss(&webview);
                }
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    if let Some(webview) = window.app_handle().get_webview_window("tray-menu") {
                        let _ = tray_popup::dismiss(&webview);
                    }
                }
                return;
            }
            if matches!(
                event,
                tauri::WindowEvent::Focused(true) | tauri::WindowEvent::ScaleFactorChanged { .. }
            ) {
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    app.state::<Controller>().refresh(&app).await;
                });
            }
            if matches!(
                event,
                tauri::WindowEvent::Destroyed | tauri::WindowEvent::CloseRequested { .. }
            ) {
                let app = window.app_handle().clone();
                tauri::async_runtime::spawn(async move {
                    app.state::<Controller>().set_recording(&app, false);
                });
            }
        })
}

fn activate_second_instance(app: &AppHandle, arguments: &[String]) {
    if !autostart::is_autostart(arguments) {
        let _ = show_window(app, false);
    }
}

fn main() {
    let settings = PathBuf::from(std::env::var_os("APPDATA").expect("APPDATA unavailable"))
        .join("Lumiere/settings.json");
    builder(Paths {
        host: host_path().expect("Application executable location unavailable"),
        settings,
    })
    .build(tauri::generate_context!())
    .expect("Lumiere shell failed")
    .run(|_, event| {
        if let tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
}
