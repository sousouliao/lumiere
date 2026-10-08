//! Explicit Windows GUI fixture; never included in the shipped executable.
use super::*;
use serde_json::json;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use webview2_com::{
    CapturePreviewCompletedHandler, ExecuteScriptCompletedHandler,
    Microsoft::Web::WebView2::Win32::COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
};
use windows::{
    Win32::{
        System::{
            Com::{STGM_CREATE, STGM_READWRITE, STGM_SHARE_EXCLUSIVE},
            ProcessStatus::{
                GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
            },
            Threading::{GetCurrentProcess, GetProcessHandleCount},
        },
        UI::Shell::SHCreateStreamOnFileEx,
    },
    core::HSTRING,
};

fn script(window: &tauri::WebviewWindow, source: &str) -> Value {
    let (tx, rx) = mpsc::channel();
    let source = source.to_owned();
    window
        .with_webview(move |webview| {
            let callback =
                ExecuteScriptCompletedHandler::create(Box::new(move |status, result| {
                    let value = status.ok().map(|()| result);
                    let _ = tx.send(value);
                    Ok(())
                }));
            // SAFETY: Tauri invokes this closure on the owning WebView STA; callback and
            // source remain owned until ExecuteScript copies them, and no handle escapes.
            unsafe {
                webview
                    .controller()
                    .CoreWebView2()
                    .unwrap()
                    .ExecuteScript(&HSTRING::from(source), &callback)
                    .unwrap();
            }
        })
        .unwrap();
    serde_json::from_str(&rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap()).unwrap()
}

fn preview(window: &tauri::WebviewWindow, path: PathBuf) {
    let (tx, rx) = mpsc::channel();
    window
        .with_webview(move |webview| {
            // SAFETY: Tauri calls this closure on the WebView STA. HSTRING owns the
            // terminated path and the callback retains the stream through completion.
            unsafe {
                let stream = SHCreateStreamOnFileEx(
                    &HSTRING::from(path.as_os_str()),
                    (STGM_CREATE | STGM_READWRITE | STGM_SHARE_EXCLUSIVE).0,
                    0,
                    true,
                    None,
                )
                .unwrap();
                let retained = stream.clone();
                let callback = CapturePreviewCompletedHandler::create(Box::new(move |status| {
                    drop(retained);
                    let _ = tx.send(status.ok());
                    Ok(())
                }));
                webview
                    .controller()
                    .CoreWebView2()
                    .unwrap()
                    .CapturePreview(
                        COREWEBVIEW2_CAPTURE_PREVIEW_IMAGE_FORMAT_PNG,
                        &stream,
                        &callback,
                    )
                    .unwrap();
            }
        })
        .unwrap();
    rx.recv_timeout(Duration::from_secs(10)).unwrap().unwrap();
}
fn wait_for(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !condition() {
        assert!(Instant::now() < deadline, "GUI fixture timed out");
        std::thread::sleep(Duration::from_millis(40));
    }
}

fn resources() -> Value {
    let mut handles = 0;
    let mut memory = PROCESS_MEMORY_COUNTERS_EX::default();
    // SAFETY: pseudo handle references this process; both output structures are
    // valid writable storage with the exact size required by the APIs.
    unsafe {
        GetProcessHandleCount(GetCurrentProcess(), &mut handles).unwrap();
        GetProcessMemoryInfo(
            GetCurrentProcess(),
            (&mut memory as *mut PROCESS_MEMORY_COUNTERS_EX).cast::<PROCESS_MEMORY_COUNTERS>(),
            size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
        )
        .unwrap();
    }
    json!({"handles":handles,"privateBytes":memory.PrivateUsage,"workingSet":memory.WorkingSetSize})
}

fn exercise(app: &AppHandle, output: &std::path::Path) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    assert!(app.webview_windows().is_empty());
    let result = tauri::async_runtime::block_on(
        app.state::<Controller>().capture(app, CaptureMode::Display),
    );
    assert_eq!(result["status"], "success");
    assert!(
        app.webview_windows().is_empty(),
        "quiet capture created a WebView"
    );
    let host_id =
        tauri::async_runtime::block_on(app.state::<Controller>().host_process_id()).unwrap();
    let controller = app.state::<Controller>();
    assert_eq!(
        controller.set_shortcut(app, CaptureMode::Region, Some("Control+Alt+F23".into()))["status"],
        "success"
    );
    assert!(app.global_shortcut().is_registered("Ctrl+Alt+F23"));
    assert_eq!(
        controller.set_shortcut(app, CaptureMode::Display, Some("Control+Alt+F23".into()))["status"],
        "failed"
    );
    controller.set_recording(app, true);
    assert!(!app.global_shortcut().is_registered("Ctrl+Alt+F23"));
    controller.set_recording(app, false);
    assert!(app.global_shortcut().is_registered("Ctrl+Alt+F23"));
    let settings = output.join("settings.json");
    let backup = output.join("settings.rollback.json");
    std::fs::rename(&settings, &backup).unwrap();
    std::fs::create_dir(&settings).unwrap();
    assert_eq!(
        controller.set_shortcut(app, CaptureMode::Region, Some("Control+Alt+F24".into()))["status"],
        "failed"
    );
    assert!(app.global_shortcut().is_registered("Ctrl+Alt+F23"));
    assert!(!app.global_shortcut().is_registered("Ctrl+Alt+F24"));
    std::fs::remove_dir(&settings).unwrap();
    std::fs::rename(&backup, &settings).unwrap();
    let old_host = tauri::async_runtime::block_on(controller.host_process_id()).unwrap();
    assert_eq!(old_host, host_id);
    tauri::async_runtime::block_on(controller.prepare_update()).unwrap();
    assert!(controller.shortcuts_suspended());
    assert!(tauri::async_runtime::block_on(controller.host_process_id()).is_none());
    assert_eq!(
        tauri::async_runtime::block_on(controller.capture(app, CaptureMode::Display))["status"],
        "failed"
    );
    controller.finish_update_exit();
    controller.resume_after_update_failure();
    controller.resume_notifications();
    assert_ne!(
        tauri::async_runtime::block_on(controller.host_process_id()).unwrap(),
        old_host
    );
    let host_id = tauri::async_runtime::block_on(controller.host_process_id()).unwrap();
    let mut samples = Vec::new();
    let cycles = std::env::var("LUMIERE_GUI_CYCLES")
        .ok()
        .map(|value| value.parse::<usize>().unwrap())
        .unwrap_or(30);
    assert!((1..=30).contains(&cycles));
    for cycle in 0..cycles {
        show_window(app, cycle % 2 == 1).unwrap();
        let window = app.get_webview_window("main").unwrap();
        wait_for(|| window.is_visible().unwrap_or(false));
        wait_for(|| {
            script(
                &window,
                "Boolean(window.lumierePlatform && document.querySelector('main'))",
            ) == true
        });
        // Mounted markup precedes asynchronous native snapshots. Capture the
        // settled product state, not the temporary disabled/loading surface.
        wait_for(|| {
            script(
                &window,
                if cycle % 2 == 0 {
                    "(() => { const buttons = [...document.querySelectorAll('.capture-action')]; return buttons.length === 2 && buttons.every(button => !button.disabled); })()"
                } else {
                    "Boolean(document.querySelector('button[aria-label^=\"Choose save folder.\"]:not(:disabled)'))"
                },
            ) == true
        });
        let title = script(&window, "document.querySelector('main').className");
        assert_eq!(
            title,
            if cycle % 2 == 0 {
                "app-shell"
            } else {
                "settings-shell settings-shell--windows"
            }
        );
        if cycle < 2 {
            preview(
                &window,
                output.join(if cycle == 0 {
                    "capture.png"
                } else {
                    "settings.png"
                }),
            );
        }
        if cycle == 0 {
            let metrics = script(
                &window,
                "({width:innerWidth,height:innerHeight,dpr:devicePixelRatio})",
            );
            std::fs::write(
                output.join("viewport.json"),
                serde_json::to_vec_pretty(&metrics).unwrap(),
            )
            .unwrap();
            script(
                &window,
                "document.querySelector('[aria-label=Maximize]').click(); true",
            );
            wait_for(|| window.is_maximized().unwrap_or(false));
            wait_for(|| {
                script(
                    &window,
                    "Boolean(document.querySelector('[aria-label=Restore]'))",
                ) == true
            });
            script(
                &window,
                "document.querySelector('[aria-label=Restore]').click(); true",
            );
            wait_for(|| !window.is_maximized().unwrap_or(true));
        }
        if cycle == 1 {
            script(
                &window,
                "window.__fixture=null; window.lumierePlatform.setHdrStatusReminders(false).then(x => window.__fixture=x).catch(e => window.__fixture={error:String(e)}); true",
            );
            wait_for(|| !script(&window, "window.__fixture").is_null());
            assert_eq!(
                script(&window, "window.__fixture.hdrStatusReminders"),
                false
            );
            assert_eq!(
                script(&window, "typeof window.__fixture.saveDirectory"),
                "string"
            );
        }
        if cycle == 2 {
            controller.set_recording(app, true);
        }
        script(
            &window,
            "document.querySelector('[aria-label=Close]').click(); true",
        );
        wait_for(|| app.webview_windows().is_empty());
        wait_for(|| !controller.shortcuts_suspended());
        std::thread::sleep(Duration::from_millis(250));
        samples.push(json!({"cycle":cycle+1,"resources":resources()}));
    }
    let persisted: Value =
        serde_json::from_slice(&std::fs::read(output.join("settings.json")).unwrap()).unwrap();
    assert_eq!(persisted["hdrStatusReminders"], false);
    assert!(app.global_shortcut().is_registered("Ctrl+Alt+F23"));
    assert_eq!(
        tauri::async_runtime::block_on(controller.host_process_id()),
        Some(host_id)
    );
    if cycles == 30 {
        assert!(
            samples[29]["resources"]["handles"].as_u64().unwrap()
                <= samples[3]["resources"]["handles"].as_u64().unwrap() + 8
        );
        assert!(
            samples[29]["resources"]["privateBytes"].as_u64().unwrap()
                <= samples[3]["resources"]["privateBytes"].as_u64().unwrap() + 32 * 1024 * 1024
        );
    }
    std::fs::write(
        output.join(format!("cycles-{cycles}.json")),
        serde_json::to_vec_pretty(&samples).unwrap(),
    )
    .unwrap();
}

// Explicit acceptance mode pauses at matching idle/visible/closed states so an
// external observer can sum the entire process tree, including WebView2 and Host.
fn acceptance_phase(output: &std::path::Path, phase: &str) {
    std::fs::write(
        output.join("phase.json"),
        serde_json::to_vec(&json!({"phase":phase,"pid":std::process::id()})).unwrap(),
    )
    .unwrap();
    let acknowledgment = output.join(format!("{phase}.ack"));
    wait_for(|| acknowledgment.exists());
}

fn acceptance(app: &AppHandle, output: &std::path::Path) {
    let controller = app.state::<Controller>();
    wait_for(|| tauri::async_runtime::block_on(controller.host_process_id()).is_some());
    acceptance_phase(output, "idle");
    for (settings, phase) in [(false, "main"), (true, "settings")] {
        show_window(app, settings).unwrap();
        let window = app.get_webview_window("main").unwrap();
        wait_for(|| window.is_visible().unwrap_or(false));
        wait_for(|| {
            script(
                &window,
                if settings {
                    "Boolean(document.querySelector('button[aria-label^=\"Choose save folder.\"]:not(:disabled)'))"
                } else {
                    "(() => { const b = [...document.querySelectorAll('.capture-action')]; return b.length === 2 && b.every(x => !x.disabled); })()"
                },
            ) == true
        });
        // Synthetic browser-scale comparisons must supply the reference client size;
        // changing browser DPR alone does not change the owning monitor's OS DPI.
        if let Ok(size) = std::env::var("LUMIERE_GUI_RASTER_SIZE") {
            let [width, height]: [u32; 2] = serde_json::from_str(&size).unwrap();
            assert!((400..=2000).contains(&width) && (300..=2000).contains(&height));
            window
                .set_size(tauri::PhysicalSize::new(width, height))
                .unwrap();
        }
        std::thread::sleep(Duration::from_millis(500));
        preview(&window, output.join(format!("{phase}.png")));
        std::fs::write(output.join(format!("{phase}-viewport.json")), serde_json::to_vec_pretty(&script(&window, "({width:innerWidth,height:innerHeight,dpr:devicePixelRatio,main:document.querySelector('main').className})")).unwrap()).unwrap();
        acceptance_phase(output, phase);
    }
    let window = app.get_webview_window("main").unwrap();
    script(
        &window,
        "document.querySelector('[aria-label=Close]').click(); true",
    );
    wait_for(|| app.webview_windows().is_empty());
    acceptance_phase(output, "closed");
}

#[test]
#[ignore = "requires interactive Windows/WebView2; build renderer and native Host first"]
fn native_shell_webview_lifecycle() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .unwrap();
    let output = std::env::var_os("LUMIERE_GUI_ACCEPTANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("artifacts/windows/tauri-shell"));
    std::fs::create_dir_all(&output).unwrap();
    let settings = output.join("settings.json");
    std::fs::write(&settings,serde_json::to_vec(&json!({"version":5,"outputDelivery":"folder","saveDirectory":output.join("captures"),"captureShortcuts":{"region":null,"display":null},"afterCaptureBehavior":"do-nothing","hdrStatusReminders":true})).unwrap()).unwrap();
    let app = builder(Paths {
        host: root.join(if cfg!(debug_assertions) {
            "target/debug/lumiere-windows-host.exe"
        } else {
            "target/release/lumiere-windows-host.exe"
        }),
        settings,
    })
    .any_thread()
    .build(tauri::generate_context!())
    .unwrap();
    let (tx, rx) = mpsc::channel();
    let mut started = false;
    let code = app.run_return(move |app, event| {
        if matches!(event, tauri::RunEvent::Ready) && !started {
            started = true;
            let app = app.clone();
            let tx = tx.clone();
            let output = output.clone();
            std::thread::spawn(move || {
                let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    if std::env::var_os("LUMIERE_GUI_ACCEPTANCE_DIR").is_some() {
                        acceptance(&app, &output);
                    } else {
                        exercise(&app, &output);
                    }
                }));
                tauri::async_runtime::block_on(app.state::<Controller>().shutdown());
                let _ = tx.send(outcome.map_err(|_| "Windows GUI fixture failed"));
                app.exit(0);
            });
        }
        if let tauri::RunEvent::ExitRequested {
            code: None, api, ..
        } = event
        {
            api.prevent_exit();
        }
    });
    assert_eq!(code, 0);
    rx.recv_timeout(Duration::from_secs(5)).unwrap().unwrap();
}
