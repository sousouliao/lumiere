//! Explicit hardware fixture: messages target only the child Host's own native HWND.
use serde_json::{Value, json};
use std::{
    process::Stdio,
    time::{Duration, Instant},
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use windows::{
    Win32::{
        Foundation::{CloseHandle, HANDLE, HWND, LPARAM, WPARAM},
        System::{
            ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::*,
        },
        UI::WindowsAndMessaging::*,
    },
    core::w,
};

struct Process(HANDLE);
struct Dpi(windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT);
impl Drop for Dpi {
    fn drop(&mut self) {
        // SAFETY: Restore the same fixture thread's previously valid DPI context.
        unsafe {
            windows::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(self.0);
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        // SAFETY: This fixture owns the OpenProcess handle, independently of the child process.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}
fn window(pid: u32) -> Option<HWND> {
    // SAFETY: Read-only discovery; messages are sent only after matching the child process ID.
    unsafe {
        let hwnd = FindWindowW(None, w!("Lumiere Region")).ok()?;
        let mut owner = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut owner));
        (owner == pid && IsWindowVisible(hwnd).as_bool()).then_some(hwnd)
    }
}
async fn visible(pid: u32) -> HWND {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(window) = window(pid) {
            return window;
        }
        assert!(
            Instant::now() < deadline,
            "Host did not show its native Region overlay"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}
fn post(window: HWND, message: u32, wparam: usize, x: u16, y: u16) {
    // SAFETY: Fixture verified this HWND belongs to its own child; LPARAM is the standard mouse packing.
    unsafe {
        PostMessageW(
            Some(window),
            message,
            WPARAM(wparam),
            LPARAM(((y as usize) << 16 | x as usize) as isize),
        )
        .unwrap();
    }
}
async fn read(reader: &mut BufReader<tokio::process::ChildStdout>) -> Value {
    let mut line = String::new();
    assert!(
        tokio::time::timeout(Duration::from_secs(10), reader.read_line(&mut line))
            .await
            .unwrap()
            .unwrap()
            > 0
    );
    serde_json::from_str(&line).unwrap()
}
fn counters(process: &Process) -> Value {
    let mut memory = PROCESS_MEMORY_COUNTERS {
        cb: size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        ..Default::default()
    };
    let mut handles = 0;
    // SAFETY: Valid query handle and sized, writable output structures; no process mutation.
    unsafe {
        GetProcessMemoryInfo(
            process.0,
            &mut memory,
            size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        )
        .unwrap();
        GetProcessHandleCount(process.0, &mut handles).unwrap();
        json!({"workingSet":memory.WorkingSetSize,"privateCommit":memory.PagefileUsage,"handles":handles,
            "userObjects":GetGuiResources(process.0, GR_USEROBJECTS),"gdiObjects":GetGuiResources(process.0, GR_GDIOBJECTS)})
    }
}

#[tokio::test]
#[ignore = "Real WGC/GPU overlay, synthetic selection and cancellation; run explicitly"]
async fn real_child_repeats_selection_and_cancellation_without_resource_growth() {
    // SAFETY: The current-thread Tokio fixture injects mouse coordinates in physical pixels.
    let _dpi = Dpi(unsafe {
        windows::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
            windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        )
    });
    let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/windows/rust-region");
    std::fs::create_dir_all(&folder).unwrap();
    let folder = folder.canonicalize().unwrap();
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_lumiere-windows-host"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let pid = child.id().unwrap();
    // SAFETY: Query-only process access, scoped by the fixture's Process guard.
    let process = Process(unsafe {
        OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).unwrap()
    });
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut measurements = Vec::new();
    for cycle in 0..30 {
        let id = format!("region-{cycle}");
        let started = Instant::now();
        input.write_all(format!("{}\n", json!({"version":6,"id":id,"method":"captureRegion","params":{"delivery":"folder","saveDirectory":folder}})).as_bytes()).await.unwrap();
        let hwnd = visible(pid).await;
        let preview_ms = started.elapsed().as_millis();
        input
            .write_all(
                format!(
                    "{}\n",
                    json!({"version":5,"id":"caps","method":"getCapabilities","params":{}})
                )
                .as_bytes(),
            )
            .await
            .unwrap();
        let caps = read(&mut output).await;
        assert_eq!(caps["id"], "caps");
        assert!(
            caps["result"]["captureModes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v == "region")
        );
        assert!(
            window(pid).is_some(),
            "Controls must not release an active Region"
        );
        match cycle % 4 {
            0 => {
                post(hwnd, WM_LBUTTONDOWN, 1, 100, 100);
                post(hwnd, WM_MOUSEMOVE, 1, 300, 240);
                post(hwnd, WM_LBUTTONUP, 0, 300, 240);
            }
            1 => post(hwnd, WM_KEYDOWN, 0x1b, 0, 0),
            2 => post(hwnd, WM_RBUTTONDOWN, 0, 0, 0),
            _ => {
                post(hwnd, WM_LBUTTONDOWN, 1, 100, 100);
                input.write_all(format!("{}\n", json!({"version":6,"id":"cancel","method":"cancelRegion","params":{"requestId":id}})).as_bytes()).await.unwrap();
            }
        }
        let mut response = read(&mut output).await;
        if cycle % 4 == 3 {
            let second = read(&mut output).await;
            let control = if response["id"] == "cancel" {
                std::mem::replace(&mut response, second)
            } else {
                second
            };
            assert_eq!(control["result"]["status"], "released");
        }
        assert_eq!(response["id"], id);
        assert!(
            window(pid).is_none(),
            "Response must wait for native selection cleanup and hiding"
        );
        if cycle % 4 == 0 {
            assert_eq!(
                response["result"]["status"], "completed",
                "cycle {cycle}: {response}"
            );
            let path = response["result"]["deliveries"][0]["filePath"]
                .as_str()
                .unwrap();
            let png = png::Decoder::new(std::io::Cursor::new(std::fs::read(path).unwrap()))
                .read_info()
                .unwrap();
            assert_eq!((png.info().width, png.info().height), (200, 140));
        } else {
            assert_eq!(response["result"]["status"], "cancelled");
        }
        measurements.push(json!({"cycle":cycle,"previewMs":preview_ms,"totalMs":started.elapsed().as_millis(),"resources":counters(&process)}));
    }
    // Observe asynchronous compositor/driver retirement independently of HWND teardown.
    let mut settled = Vec::new();
    for _ in 0..6 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        settled.push(counters(&process));
    }
    // EOF while the real selection window is active must also wait for native cleanup.
    input.write_all(b"{\"version\":6,\"id\":\"eof\",\"method\":\"captureRegion\",\"params\":{\"delivery\":\"clipboard\"}}\n").await.unwrap();
    visible(pid).await;
    input.shutdown().await.unwrap();
    drop(input);
    assert_eq!(read(&mut output).await["result"]["status"], "cancelled");
    assert!(window(pid).is_none());
    let result = tokio::time::timeout(Duration::from_secs(5), child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    assert!(result.status.success());
    assert!(
        result.stderr.is_empty(),
        "Host stdout/stderr contract: {:?}",
        result.stderr
    );
    std::fs::write(
        folder.join("cycles.json"),
        serde_json::to_vec_pretty(&json!({"cycles":measurements,"settled":settled})).unwrap(),
    )
    .unwrap();
    let first = &measurements[4]["resources"];
    let last = settled.last().unwrap();
    println!(
        "native resources: warm={first}, immediate={}, settled={last}",
        measurements[29]["resources"]
    );
    assert!(last["handles"].as_u64().unwrap() <= first["handles"].as_u64().unwrap() + 8);
    assert!(last["userObjects"].as_u64().unwrap() <= first["userObjects"].as_u64().unwrap() + 2);
    assert!(last["gdiObjects"].as_u64().unwrap() <= first["gdiObjects"].as_u64().unwrap() + 2);
    assert!(
        last["privateCommit"].as_u64().unwrap()
            <= first["privateCommit"].as_u64().unwrap() + 64 * 1024 * 1024
    );
    println!(
        "30 native cycles + active EOF passed; first={first}, last={last}; evidence={}",
        folder.join("cycles.json").display()
    );
}

#[tokio::test]
#[ignore = "Queries real D3D capability resources; run explicitly"]
async fn repeated_capabilities_release_their_native_probe_resources() {
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_lumiere-windows-host"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    // SAFETY: Query-only handle for this test's child, released by Process.
    let process = Process(unsafe {
        OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            child.id().unwrap(),
        )
        .unwrap()
    });
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut snapshots = Vec::new();
    for _ in 0..30 {
        input
            .write_all(
                b"{\"version\":5,\"id\":\"caps\",\"method\":\"getCapabilities\",\"params\":{}}\n",
            )
            .await
            .unwrap();
        assert_eq!(read(&mut output).await["result"]["hostStatus"], "available");
        snapshots.push(counters(&process));
    }
    tokio::time::sleep(Duration::from_secs(1)).await;
    println!(
        "capabilities: first={}, last={}, settled={}",
        snapshots[4],
        snapshots[29],
        counters(&process)
    );
    drop(input);
    assert!(child.wait().await.unwrap().success());
    assert!(
        snapshots[29]["handles"].as_u64().unwrap() <= snapshots[4]["handles"].as_u64().unwrap() + 8
    );
}
