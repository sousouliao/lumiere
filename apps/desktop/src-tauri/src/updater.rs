use serde_json::{Value, json};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

pub struct Updates {
    state: Mutex<Value>,
    operation: tokio::sync::Mutex<Payload>,
}
#[derive(Default)]
struct Payload {
    update: Option<Update>,
    verified: Option<Vec<u8>>,
}
impl Updates {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(
                json!({"status": if cfg!(debug_assertions) {"disabled"} else {"idle"}}),
            ),
            operation: tokio::sync::Mutex::new(Payload::default()),
        }
    }
    pub fn snapshot(&self, app: &AppHandle) -> Value {
        json!({"currentVersion": app.package_info().version.to_string(), "windowsUpdate": self.state.lock().unwrap().clone()})
    }
    fn check_result(&self, app: &AppHandle, status: &str, version: Option<&str>) -> Value {
        let mut result = self.snapshot(app);
        result["status"] = json!(status);
        if let Some(version) = version {
            result["availableVersion"] = json!(version);
        }
        result
    }
    fn set(&self, app: &AppHandle, state: Value) {
        *self.state.lock().unwrap() = state;
        let _ = app.emit("update-changed", self.snapshot(app));
    }
    fn failure(&self, app: &AppHandle, retry: &str, version: Option<&str>) {
        let mut state = json!({"status":"failed", "message": match retry {
            "check" => "Couldn?t check for updates",
            "download" => "Couldn?t download or verify the update",
            _ => "Couldn?t start the installer",
        }, "retry":retry});
        if let Some(version) = version {
            state["availableVersion"] = json!(version);
        }
        self.set(app, state);
    }
    pub async fn check(&self, app: &AppHandle) -> Value {
        let current = app.package_info().version.to_string();
        if cfg!(debug_assertions) {
            return json!({"status":"idle", "currentVersion":current, "windowsUpdate":{"status":"disabled"}});
        }
        let Ok(mut payload) = self.operation.try_lock() else {
            return json!({"status":"idle", "currentVersion":current, "windowsUpdate":self.state.lock().unwrap().clone()});
        };
        // A scheduled check must never discard an explicitly downloaded installer.
        if payload.verified.is_some() {
            return self.check_result(
                app,
                "available",
                Some(&payload.update.as_ref().unwrap().version),
            );
        }
        self.set(app, json!({"status":"checking"}));
        let install_directory = match std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(std::path::Path::to_path_buf))
        {
            Some(path) => path,
            None => {
                self.failure(app, "check", None);
                return self.check_result(app, "failed", None);
            }
        };
        let result = match app
            .updater_builder()
            .timeout(std::time::Duration::from_secs(30))
            // NSIS consumes /D's remaining text; keep it last. Bind handoff to
            // this installation even if another process changes registration.
            .installer_arg(format!("/D={}", install_directory.display()))
            .on_before_exit({
                let app = app.clone();
                move || app.state::<crate::Controller>().finish_update_exit()
            })
            .build()
        {
            Ok(updater) => updater.check().await,
            Err(error) => Err(error),
        };
        match result {
            Ok(Some(update)) => {
                let version = update.version.clone();
                payload.update = Some(update);
                self.set(
                    app,
                    json!({"status":"available", "availableVersion":version}),
                );
                self.check_result(app, "available", Some(&version))
            }
            Ok(None) => {
                payload.update = None;
                self.set(app, json!({"status":"up-to-date"}));
                self.check_result(app, "up-to-date", None)
            }
            Err(error) => {
                eprintln!(
                    "{}",
                    json!({"level":"warn", "event":"update-check-failed", "error":error.to_string()})
                );
                self.failure(app, "check", None);
                self.check_result(app, "failed", None)
            }
        }
    }
    pub async fn download(&self, app: &AppHandle) -> Result<Value, String> {
        let mut payload = self
            .operation
            .try_lock()
            .map_err(|_| "An update operation is in progress")?;
        let update = payload.update.as_ref().ok_or("Check for updates first")?;
        let version = update.version.clone();
        self.set(
            app,
            json!({"status":"downloading", "availableVersion":version, "percent":0}),
        );
        let mut received = 0u64;
        let mut last_percent = 0u64;
        let bytes = update.download(|chunk, total| {
            received += chunk as u64;
            let percent = total.filter(|total| *total > 0).map(|total| received.saturating_mul(100) / total).unwrap_or(0).min(99);
            if percent != last_percent {
                last_percent = percent;
                self.set(app, json!({"status":"downloading", "availableVersion":version, "percent":percent}));
            }
        }, || {}).await;
        match bytes {
            Ok(bytes) => {
                // Official plugin verifies both minisign bytes and signed version before return.
                payload.verified = Some(bytes);
                self.set(app, json!({"status":"ready", "availableVersion":version}));
            }
            Err(error) => {
                payload.verified = None;
                eprintln!(
                    "{}",
                    json!({"level":"warn", "event":"update-verification-failed", "error":error.to_string()})
                );
                self.failure(app, "download", Some(&version));
            }
        }
        Ok(self.snapshot(app))
    }
    pub async fn install(&self, app: &AppHandle) -> Result<Value, String> {
        let payload = self
            .operation
            .try_lock()
            .map_err(|_| "An update operation is in progress")?;
        let update = payload.update.as_ref().ok_or("Download an update first")?;
        let bytes = payload
            .verified
            .as_ref()
            .ok_or("Download and verify the update first")?;
        let controller = app.state::<crate::Controller>();
        controller.prepare_update().await?;
        self.set(
            app,
            json!({"status":"installing", "availableVersion":update.version}),
        );
        // Windows install hands off the verified NSIS bytes and exits on success.
        // EOF/join has already retired the native Host before any file replacement.
        if let Err(error) = update.install(bytes) {
            controller.resume_after_update_failure();
            controller.resume_notifications();
            controller.refresh(app).await;
            eprintln!(
                "{}",
                json!({"level":"warn", "event":"update-install-failed", "error":error.to_string()})
            );
            self.failure(app, "install", Some(&update.version));
        }
        Ok(self.snapshot(app))
    }
}
pub fn schedule(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        loop {
            if app
                .state::<crate::Controller>()
                .quitting
                .load(std::sync::atomic::Ordering::Acquire)
            {
                break;
            }
            app.state::<Updates>().check(&app).await;
            tokio::time::sleep(std::time::Duration::from_secs(6 * 60 * 60)).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
    };
    #[test]
    #[ignore = "requires signed local NSIS and Windows runtime; optional isolated subprocess handoff"]
    fn official_updater_accepts_signed_bytes_and_rejects_tampering() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let metadata: Value = serde_json::from_slice(
            &std::fs::read(root.join("artifacts/windows/release/latest.json")).unwrap(),
        )
        .unwrap();
        let version = metadata["version"].as_str().unwrap().to_owned();
        let bytes = Arc::new(
            std::fs::read(root.join(format!(
                "artifacts/windows/release/Lumiere-Setup-{version}-x64.exe"
            )))
            .unwrap(),
        );
        let signature = metadata["platforms"]["windows-x86_64"]["signature"]
            .as_str()
            .unwrap()
            .to_owned();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        server.set_nonblocking(true).unwrap();
        let origin = format!("http://{}", server.local_addr().unwrap());
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let bytes = bytes.clone();
            let stop = stop.clone();
            let origin = origin.clone();
            let version = version.clone();
            std::thread::spawn(move || {
                while !stop.load(Ordering::Acquire) {
                    let (mut stream, _) = match server.accept() {
                        Ok(value) => value,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(std::time::Duration::from_millis(10));
                            continue;
                        }
                        Err(error) => panic!("{error}"),
                    };
                    // Accepted sockets inherit the nonblocking listener on Windows.
                    // Read this fixture's local HTTP request synchronously with a timeout.
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                        .unwrap();
                    let mut request = [0u8; 4096];
                    let size = stream.read(&mut request).unwrap();
                    let path = std::str::from_utf8(&request[..size])
                        .unwrap()
                        .split_whitespace()
                        .nth(1)
                        .unwrap();
                    let body = if let Some(mode) = path.strip_prefix("/metadata/") {
                        serde_json::to_vec(&json!({"version":if mode=="version" {"999.0.0"}else{&version},"platforms":{"windows-x86_64":{"signature":if mode=="signature" {"invalid signature"}else{&signature},"url":format!("{origin}/installer/{mode}")}}})).unwrap()
                    } else {
                        let mut body = bytes.as_ref().clone();
                        if path.ends_with("tampered") {
                            let index = body.len() / 2;
                            body[index] ^= 1;
                        }
                        body
                    };
                    let header = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    stream.write_all(header.as_bytes()).unwrap();
                    stream.write_all(&body).unwrap();
                }
            })
        };
        let mut context = tauri::generate_context!();
        context.config_mut().plugins.0.get_mut("updater").unwrap()["dangerousInsecureTransportProtocol"] =
            json!(true);
        let app = tauri::Builder::default()
            .plugin(tauri_plugin_updater::Builder::new().build())
            .any_thread()
            .build(context)
            .unwrap();
        let mut verified_update = None;
        for mode in ["valid", "tampered", "version", "signature"] {
            let updater = app
                .handle()
                .updater_builder()
                .target("windows-x86_64")
                .no_proxy()
                .version_comparator(|_, _| true)
                .installer_args(
                    std::env::var_os("LUMIERE_TEST_UPDATE_HANDOFF")
                        .into_iter()
                        .map(|path| format!("/D={}", std::path::PathBuf::from(path).display())),
                )
                .restart_after_install(false)
                .on_before_exit({
                    let app = app.handle().clone();
                    move || {
                        if let Some(controller) = app.try_state::<crate::Controller>() {
                            controller.finish_update_exit();
                        }
                    }
                })
                .endpoints(vec![format!("{origin}/metadata/{mode}").parse().unwrap()])
                .unwrap()
                .build()
                .unwrap();
            tauri::async_runtime::block_on(async {
                let update = updater.check().await.unwrap().unwrap();
                let result = update.download(|_, _| {}, || {}).await;
                if mode == "valid" {
                    assert_eq!(result.unwrap(), *bytes);
                    verified_update = Some(update);
                } else {
                    assert!(result.is_err(), "{mode} must be rejected");
                }
            });
        }
        stop.store(true, Ordering::Release);
        thread.join().unwrap();
        if let Some(destination) = std::env::var_os("LUMIERE_TEST_UPDATE_HANDOFF") {
            let destination = std::path::PathBuf::from(destination);
            app.manage(crate::Controller::new(
                destination.join("lumiere-windows-host.exe"),
                destination.join("fixture-settings.json"),
                destination.join("captures"),
            ));
            let controller = app.state::<crate::Controller>();
            let host_pid = tauri::async_runtime::block_on(controller.host_process_id()).unwrap();
            tauri::async_runtime::block_on(controller.prepare_update()).unwrap();
            std::fs::write(
                destination.join("fixture-host.json"),
                serde_json::to_vec(&json!({"hostPid":host_pid})).unwrap(),
            )
            .unwrap();
            // An explicit subprocess fixture only. Official Windows install exits
            // this process after handing the verified installer to NSIS.
            verified_update.unwrap().install(bytes.as_ref()).unwrap();
            panic!("Windows updater did not exit after installer handoff");
        }
    }
}
