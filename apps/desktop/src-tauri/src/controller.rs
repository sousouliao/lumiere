use crate::{
    host::Host,
    settings::{AfterCapture, Settings, Shortcuts, normalize_shortcut},
};
use lumiere_capture_contract::{CaptureMode, Delivery};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

pub struct Controller {
    data: Mutex<Data>,
    host: tokio::sync::Mutex<Option<Arc<Host>>>,
    host_path: PathBuf,
    settings_path: PathBuf,
    default_directory: PathBuf,
    notifications: crate::notification::Notifications,
    pub quitting: AtomicBool,
}
struct Data {
    settings: Settings,
    registered: Shortcuts,
    recording: bool,
    updating: bool,
    active: Option<(CaptureMode, String)>,
    completion: Option<Value>,
    completion_id: u64,
    surface: Option<Value>,
}
impl Controller {
    pub fn new(host_path: PathBuf, settings_path: PathBuf, default_directory: PathBuf) -> Self {
        Self {
            data: Mutex::new(Data {
                settings: Settings::load(&settings_path),
                registered: Shortcuts::default(),
                recording: false,
                updating: false,
                active: None,
                completion: None,
                completion_id: 0,
                surface: None,
            }),
            host: tokio::sync::Mutex::new(None),
            host_path,
            settings_path,
            default_directory,
            notifications: crate::notification::Notifications::new(),
            quitting: AtomicBool::new(false),
        }
    }
    async fn host(&self) -> Result<Arc<Host>, String> {
        if self.quitting.load(Ordering::Acquire) || self.data.lock().unwrap().updating {
            return Err("Lumiere is shutting down".into());
        }
        let mut slot = self.host.lock().await;
        if self.data.lock().unwrap().updating {
            return Err("Lumiere is updating".into());
        }
        if slot.as_ref().is_some_and(|host| !host.is_alive())
            && let Some(host) = slot.take()
        {
            host.shutdown().await;
        }
        if slot.is_none() {
            *slot = Some(Arc::new(Host::launch(&self.host_path)?));
        }
        Ok(slot.as_ref().expect("Host was initialized").clone())
    }
    pub async fn shutdown(&self) {
        self.quitting.store(true, Ordering::Release);
        self.notifications.shutdown();
        if let Some(host) = self.host.lock().await.take() {
            host.shutdown().await;
        }
    }
    pub async fn prepare_update(&self) -> Result<(), String> {
        {
            let mut data = self.data.lock().map_err(|_| "Capture state unavailable")?;
            if data.active.is_some() || data.updating || self.quitting.load(Ordering::Acquire) {
                return Err("Finish the capture before restarting to update".into());
            }
            data.updating = true;
        }
        self.notifications.clear();
        let mut slot = self.host.lock().await;
        if let Some(host) = slot.take()
            && let Err(error) = host.shutdown_checked().await
        {
            *slot = Some(host);
            self.data.lock().unwrap().updating = false;
            return Err(format!("Native capture Host could not exit: {error}"));
        }
        Ok(())
    }
    pub fn resume_after_update_failure(&self) {
        self.data.lock().unwrap().updating = false;
    }
    pub fn resume_notifications(&self) {
        self.notifications.restart();
    }
    pub fn finish_update_exit(&self) {
        self.notifications.shutdown();
    }
    #[cfg(test)]
    pub async fn host_process_id(&self) -> Option<u32> {
        self.host().await.ok()?.process_id().await
    }
    async fn request(&self, version: u8, method: &str, params: Value) -> Result<Value, String> {
        let host = self.host().await?;
        let result = host
            .request(host.request_id(), version, method, params)
            .await;
        if result.is_err() {
            let mut slot = self.host.lock().await;
            if slot
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(current, &host))
            {
                slot.take();
                host.shutdown().await;
            }
        }
        result
    }
    pub fn activity(&self) -> Value {
        let data = self.data.lock().expect("Controller state poisoned");
        json!({"activeMode":data.active.as_ref().map(|(mode,_)|mode),"lastCompletion":data.completion})
    }
    pub fn settings_snapshot(&self) -> Value {
        let data = self.data.lock().expect("Controller state poisoned");
        let mut snapshot = serde_json::to_value(&data.settings).expect("Settings serialize");
        snapshot.as_object_mut().unwrap().remove("version");
        snapshot["saveDirectory"] = json!(
            data.settings
                .save_directory
                .as_ref()
                .unwrap_or(&self.default_directory)
        );
        snapshot["availableOutputDeliveries"] = json!(["clipboard", "folder", "both"]);
        for mode in [CaptureMode::Region, CaptureMode::Display] {
            let accelerator = data.settings.capture_shortcuts.get(mode);
            let status = if accelerator.is_none() {
                "unconfigured"
            } else if accelerator == data.registered.get(mode) {
                "registered"
            } else {
                "unavailable"
            };
            snapshot["captureShortcuts"][mode_name(mode)] =
                json!({"accelerator":accelerator,"status":status});
        }
        snapshot
    }
    pub fn update_settings(
        &self,
        app: &AppHandle,
        change: impl FnOnce(&mut Settings),
    ) -> Result<Value, String> {
        {
            let mut data = self.data.lock().map_err(|_| "Settings unavailable")?;
            let mut next = data.settings.clone();
            change(&mut next);
            next.save(&self.settings_path)?;
            data.settings = next;
        }
        let snapshot = self.settings_snapshot();
        let _ = app.emit("settings-changed", &snapshot);
        self.update_tray(app);
        Ok(snapshot)
    }
    pub fn initialize_shortcuts(&self, app: &AppHandle) {
        let mut data = self.data.lock().expect("Controller state poisoned");
        for mode in [CaptureMode::Region, CaptureMode::Display] {
            if let Some(value) = data.settings.capture_shortcuts.get(mode).clone() {
                let canonical = normalize_shortcut(&value).unwrap_or(value);
                if register(app, mode, &canonical).is_ok() {
                    data.settings
                        .capture_shortcuts
                        .set(mode, Some(canonical.clone()));
                    data.registered.set(mode, Some(canonical));
                }
            }
        }
    }
    pub fn set_shortcut(
        &self,
        app: &AppHandle,
        mode: CaptureMode,
        accelerator: Option<String>,
    ) -> Value {
        let result = (|| -> Result<(), String> {
            let next = accelerator.as_deref().map(normalize_shortcut).transpose()?;
            let mut data = self.data.lock().map_err(|_| "Settings unavailable")?;
            let other = if mode == CaptureMode::Region {
                CaptureMode::Display
            } else {
                CaptureMode::Region
            };
            if next.is_some() && data.settings.capture_shortcuts.get(other) == &next {
                return Err("That shortcut is already used by Lumiere.".into());
            }
            let old = data.registered.get(mode).clone();
            if old == next && data.settings.capture_shortcuts.get(mode) == &next {
                return Ok(());
            }
            if let Some(value) = &next {
                register(app, mode, value)?;
            }
            let mut settings = data.settings.clone();
            settings.capture_shortcuts.set(mode, next.clone());
            if let Err(error) = settings.save(&self.settings_path) {
                if let Some(value) = &next {
                    let _ = app
                        .global_shortcut()
                        .unregister(native_shortcut(value).as_str());
                }
                return Err(error);
            }
            if let Some(value) = old {
                let _ = app
                    .global_shortcut()
                    .unregister(native_shortcut(&value).as_str());
            }
            data.settings = settings;
            if data.recording
                && let Some(value) = &next
            {
                let _ = app
                    .global_shortcut()
                    .unregister(native_shortcut(value).as_str());
            }
            data.registered.set(mode, next);
            Ok(())
        })();
        match result {
            Ok(()) => {
                let snapshot = self.settings_snapshot();
                let _ = app.emit("settings-changed", &snapshot);
                self.update_tray(app);
                json!({"status":"success","snapshot":snapshot})
            }
            Err(message) => json!({"status":"failed","message":message}),
        }
    }
    pub fn set_recording(&self, app: &AppHandle, recording: bool) {
        let mut data = self.data.lock().expect("Controller state poisoned");
        if data.recording == recording {
            return;
        }
        data.recording = recording;
        if recording {
            for value in [&data.registered.region, &data.registered.display]
                .into_iter()
                .flatten()
            {
                let _ = app
                    .global_shortcut()
                    .unregister(native_shortcut(value).as_str());
            }
        } else {
            for mode in [CaptureMode::Region, CaptureMode::Display] {
                let next = data.settings.capture_shortcuts.get(mode).clone();
                let registered = next.filter(|value| register(app, mode, value).is_ok());
                data.registered.set(mode, registered);
            }
        }
    }
    pub fn shortcuts_suspended(&self) -> bool {
        let data = self.data.lock().expect("Controller state poisoned");
        data.recording || data.updating || self.quitting.load(Ordering::Acquire)
    }
    pub async fn surface(&self) -> Value {
        let capabilities = self.request(5, "getCapabilities", json!({})).await;
        let settings = self
            .data
            .lock()
            .expect("Controller state poisoned")
            .settings
            .clone();
        let (available, modes, hdr) = match capabilities {
            Ok(value) if value["platform"] == "windows" && value["contractVersion"] == 5 => (
                value["hostStatus"] == "available",
                value["captureModes"].clone(),
                match value["hdrCapture"].as_str() {
                    Some("supported") => "ready",
                    Some("unvalidated") => "unvalidated",
                    _ => "unavailable",
                },
            ),
            _ => (false, json!([]), "unavailable"),
        };
        let label = match settings.output_delivery {
            Delivery::Both => "Clipboard and folder",
            Delivery::Folder => "Folder",
            Delivery::Clipboard => "Clipboard",
        };
        let location = if settings.output_delivery == Delivery::Clipboard {
            "Ready for paste".into()
        } else {
            settings
                .save_directory
                .as_ref()
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or("%USERPROFILE%\\Pictures\\Lumiere".into())
        };
        let mut result = json!({"platform":"windows","hostAvailable":available,"captureModes":modes,"hdrStatus":hdr,"output":{"delivery":settings.output_delivery,"label":label,"location":location}});
        if !available {
            result["blockingNotice"] = notice(
                "critical",
                "Native capture host is unavailable",
                "Retry, or restart Lumiere if it does not come back.",
                None,
            );
        } else if settings.hdr_status_reminders && hdr != "ready" {
            result["advisoryNotice"] = notice(
                "caution",
                if hdr == "unvalidated" {
                    "This display has not been verified"
                } else {
                    "HDR-aware capture is unavailable for this display"
                },
                "The display under the pointer will use sRGB Visual Match.",
                None,
            );
        }
        result
    }
    pub async fn refresh(&self, app: &AppHandle) -> Value {
        let surface = self.surface().await;
        self.data.lock().expect("Controller state poisoned").surface = Some(surface.clone());
        let _ = app.emit("capture-surface-changed", &surface);
        self.update_tray(app);
        surface
    }
    fn update_tray(&self, app: &AppHandle) {
        let snapshot = self.settings_snapshot();
        let (surface, busy) = {
            let data = self.data.lock().expect("Controller state poisoned");
            (data.surface.clone(), data.active.is_some() || data.updating)
        };
        if let Some(surface) = surface {
            crate::update_tray(app, &surface, &snapshot, busy);
        }
    }
    pub async fn capture(&self, app: &AppHandle, mode: CaptureMode) -> Value {
        let (id, settings) = {
            let mut data = self.data.lock().expect("Controller state poisoned");
            if data.active.is_some() || data.updating || self.quitting.load(Ordering::Acquire) {
                return failed(notice(
                    "caution",
                    "A capture is already in progress",
                    "Wait for it to finish, then try again.",
                    None,
                ));
            }
            let id = format!("capture-{}", data.completion_id + 1);
            data.active = Some((mode, id.clone()));
            (id, data.settings.clone())
        };
        self.notifications.clear();
        let restore = app.get_webview_window("main").filter(|window| {
            window.is_visible().unwrap_or(false) && !window.is_minimized().unwrap_or(false)
        });
        let _ = app.emit("capture-activity-changed", self.activity());
        self.update_tray(app);
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
        let mut params = json!({"delivery":settings.output_delivery});
        if settings.output_delivery != Delivery::Clipboard
            && let Some(path) = &settings.save_directory
        {
            params["saveDirectory"] = json!(path);
        }
        let response = match self.host().await {
            Ok(host) => {
                host.request(
                    id,
                    if mode == CaptureMode::Region { 6 } else { 5 },
                    if mode == CaptureMode::Region {
                        "captureRegion"
                    } else {
                        "captureDisplay"
                    },
                    params,
                )
                .await
            }
            Err(error) => Err(error),
        };
        let result = match response {
            Ok(value) => project_result(value),
            Err(_) => failed(notice(
                "critical",
                "Capture failed",
                "Try again. Restart Lumiere if the issue continues.",
                None,
            )),
        };
        {
            let mut data = self.data.lock().expect("Controller state poisoned");
            data.active = None;
            data.completion_id += 1;
            data.completion = Some(json!({"id":data.completion_id,"mode":mode,"result":result}));
        }
        let _ = app.emit("capture-activity-changed", self.activity());
        self.update_tray(app);
        if self.quitting.load(Ordering::Acquire) {
            return result;
        }
        if mode == CaptureMode::Region
            && let Some(window) = &restore
        {
            let _ = window.show();
            let _ = window.set_focus();
        }
        let foreground = app
            .get_webview_window("main")
            .is_some_and(|window| window.is_focused().unwrap_or(false));
        if !foreground && matches!(result["status"].as_str(), Some("failed" | "partial")) {
            let id = self
                .data
                .lock()
                .expect("Controller state poisoned")
                .completion_id;
            let partial = result["status"] == "partial";
            self.notifications.show(
                app,
                id,
                if partial {
                    "Screenshot needs attention"
                } else {
                    result["notice"]["title"]
                        .as_str()
                        .unwrap_or("Capture failed")
                },
                if partial {
                    result["feedback"].as_str().unwrap_or("")
                } else {
                    result["notice"]["detail"].as_str().unwrap_or("")
                },
            );
        }
        if settings.after_capture_behavior == AfterCapture::ShowInFolder
            && let Some(path) = result["filePath"].as_str()
        {
            let _ = app.opener().reveal_item_in_dir(path);
        }
        self.refresh(app).await;
        result
    }
    pub fn completion_mode(&self, id: u64) -> Option<CaptureMode> {
        let data = self.data.lock().ok()?;
        let completion = data.completion.as_ref()?;
        if data.active.is_some() || completion["id"] != id {
            return None;
        }
        match completion["mode"].as_str()? {
            "region" => Some(CaptureMode::Region),
            "display" => Some(CaptureMode::Display),
            _ => None,
        }
    }
}

fn mode_name(mode: CaptureMode) -> &'static str {
    if mode == CaptureMode::Region {
        "region"
    } else {
        "display"
    }
}
fn native_shortcut(value: &str) -> String {
    value
        .replace("Command+", "Super+")
        .replace("Control+", "Ctrl+")
}
fn register(app: &AppHandle, mode: CaptureMode, value: &str) -> Result<(), String> {
    app.global_shortcut()
        .on_shortcut(native_shortcut(value).as_str(), move |app, _, event| {
            if event.state == ShortcutState::Pressed {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    if !app.state::<Controller>().shortcuts_suspended() {
                        app.state::<Controller>().capture(&app, mode).await;
                    }
                });
            }
        })
        .map_err(|_| "That shortcut is already used by another app.".into())
}
fn notice(tone: &str, title: &str, detail: &str, recovery: Option<&str>) -> Value {
    let mut value = json!({"tone":tone,"title":title,"detail":detail});
    if let Some(recovery) = recovery {
        value["recovery"] = json!(recovery);
    }
    value
}
fn failed(notice: Value) -> Value {
    json!({"status":"failed","feedback":notice["title"],"notice":notice})
}
fn project_result(value: Value) -> Value {
    match value["status"].as_str() {
        Some("cancelled") => json!({"status":"cancelled","feedback":"Capture cancelled"}),
        Some("failed") => {
            let failure = &value["failure"]["code"];
            failed(match failure.as_str() {
                Some("capture-unavailable") => notice(
                    "caution",
                    "Capture failed",
                    "The display may have changed. Try again.",
                    None,
                ),
                Some("host-unavailable") => notice(
                    "critical",
                    "Native capture host is unavailable",
                    "Retry, or restart Lumiere if it does not come back.",
                    None,
                ),
                Some("delivery-unavailable" | "delivery-failed") => notice(
                    "caution",
                    "Couldn’t deliver capture",
                    "Check the output destination, then try again.",
                    Some("output"),
                ),
                _ => notice(
                    "critical",
                    "Capture failed",
                    "Try again. Restart Lumiere if the issue continues.",
                    None,
                ),
            })
        }
        Some("completed") => {
            let Some(deliveries) = value["deliveries"].as_array() else {
                return failed(notice(
                    "critical",
                    "Capture failed",
                    "Try again. Restart Lumiere if the issue continues.",
                    None,
                ));
            };
            let clipboard = deliveries
                .iter()
                .any(|item| item["target"] == "clipboard" && item["status"] == "success");
            let path = deliveries
                .iter()
                .find(|item| item["target"] == "folder" && item["status"] == "success")
                .and_then(|item| item["filePath"].as_str());
            let folder = path.is_some();
            let mut result = if !deliveries.is_empty()
                && deliveries.iter().all(|item| item["status"] == "success")
            {
                let name = path
                    .and_then(|path| {
                        PathBuf::from(path)
                            .parent()
                            .and_then(|parent| parent.file_name())
                            .map(|name| name.to_string_lossy().into_owned())
                    })
                    .unwrap_or("Lumiere".into());
                json!({"status":"success","feedback":if clipboard&&folder {format!("Copied and saved to “{name}”")}else if clipboard {"Copied to clipboard".into()}else{format!("Saved to “{name}”")}})
            } else if clipboard || folder {
                let feedback = if clipboard {
                    "Copied to clipboard, but couldn’t save the file"
                } else {
                    "Saved the file, but couldn’t copy it"
                };
                json!({"status":"partial","feedback":feedback,"notice":notice("caution",feedback,if clipboard {"Choose a writable folder, then take a new capture."}else{"Take a new capture to try copying again. Your saved file is unchanged."},if clipboard {Some("folder")}else{None})})
            } else {
                failed(notice(
                    "caution",
                    "Couldn’t deliver capture",
                    "Check the output destination, then try again.",
                    Some("output"),
                ))
            };
            if let Some(path) = path {
                result["filePath"] = json!(path);
            }
            result
        }
        _ => failed(notice(
            "critical",
            "Capture failed",
            "Try again. Restart Lumiere if the issue continues.",
            None,
        )),
    }
}
