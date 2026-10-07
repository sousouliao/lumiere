use lumiere_capture_contract::{Capabilities, CaptureMode, CaptureOutcome, CaptureParams, Failure};
use lumiere_capture_windows::{Cancellation, CaptureEngine};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

struct ControlledEngine {
    started: AtomicBool,
    released: AtomicBool,
}
impl ControlledEngine {
    fn new() -> Self {
        Self {
            started: AtomicBool::new(false),
            released: AtomicBool::new(false),
        }
    }
}
impl CaptureEngine for ControlledEngine {
    fn capabilities(&self, version: u8) -> Capabilities {
        Capabilities::unavailable(version, Failure::capture("Test backend"))
    }
    fn capture(&self, _: CaptureMode, _: CaptureParams, cancel: Cancellation) -> CaptureOutcome {
        self.started.store(true, Ordering::Release);
        while !cancel.is_cancelled() {
            std::thread::sleep(Duration::from_millis(1));
        }
        self.released.store(true, Ordering::Release);
        CaptureOutcome::Cancelled
    }
}

async fn line(reader: &mut BufReader<tokio::io::DuplexStream>) -> Value {
    let mut result = String::new();
    tokio::time::timeout(Duration::from_secs(3), reader.read_line(&mut result))
        .await
        .unwrap()
        .unwrap();
    serde_json::from_str(&result).unwrap()
}

#[tokio::test]
async fn immediate_and_multiple_cancellation_wait_for_release_without_blocking_controls() {
    let (mut input, host_input) = tokio::io::duplex(4096);
    let (host_output, output) = tokio::io::duplex(4096);
    let engine = Arc::new(ControlledEngine::new());
    let task = tokio::spawn(lumiere_windows_host::serve(
        host_input,
        host_output,
        engine.clone(),
    ));
    let mut reader = BufReader::new(output);
    input.write_all(concat!(
        "{\"version\":6,\"id\":\"capture\",\"method\":\"captureRegion\",\"params\":{\"delivery\":\"clipboard\"}}\n",
        "{\"version\":5,\"id\":\"caps\",\"method\":\"getCapabilities\",\"params\":{}}\n",
        "{\"version\":6,\"id\":\"cancel-a\",\"method\":\"cancelRegion\",\"params\":{\"requestId\":\"capture\"}}\n",
        "{\"version\":6,\"id\":\"cancel-b\",\"method\":\"cancelRegion\",\"params\":{\"requestId\":\"capture\"}}\n"
    ).as_bytes()).await.unwrap();
    let mut responses = Vec::new();
    for _ in 0..4 {
        let response = line(&mut reader).await;
        if response["id"] != "caps" {
            assert!(engine.released.load(Ordering::Acquire));
        }
        responses.push(response);
    }
    for id in ["capture", "caps", "cancel-a", "cancel-b"] {
        assert_eq!(responses.iter().filter(|r| r["id"] == id).count(), 1);
    }
    assert_eq!(
        responses.iter().find(|r| r["id"] == "capture").unwrap()["result"]["status"],
        "cancelled"
    );
    input.shutdown().await.unwrap();
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn overlap_is_rejected_and_eof_releases_the_active_operation() {
    let (mut input, host_input) = tokio::io::duplex(4096);
    let (host_output, output) = tokio::io::duplex(4096);
    let engine = Arc::new(ControlledEngine::new());
    let task = tokio::spawn(lumiere_windows_host::serve(
        host_input,
        host_output,
        engine.clone(),
    ));
    let mut reader = BufReader::new(output);
    for id in ["first", "second"] {
        input.write_all(format!("{}\n",json!({"version":6,"id":id,"method":"captureRegion","params":{"delivery":"clipboard"}})).as_bytes()).await.unwrap();
    }
    let rejected = line(&mut reader).await;
    assert_eq!(rejected["id"], "second");
    assert_eq!(rejected["result"]["failure"]["code"], "capture-unavailable");
    assert!(!engine.released.load(Ordering::Acquire));
    input.shutdown().await.unwrap();
    let cancelled = line(&mut reader).await;
    assert_eq!(cancelled["id"], "first");
    assert_eq!(cancelled["result"]["status"], "cancelled");
    tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(engine.released.load(Ordering::Acquire));
}

#[tokio::test]
async fn inactive_cancel_is_idempotent_and_real_child_speaks_only_json_lines() {
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_lumiere-windows-host"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input.write_all(concat!(
        "{\"version\":6,\"id\":\"cancel\",\"method\":\"cancelRegion\",\"params\":{\"requestId\":\"finished\"}}\n",
        "{\"version\":5,\"id\":\"caps\",\"method\":\"getCapabilities\",\"params\":{}}\n",
        "{\"version\":6,\"id\":\"bad\",\"method\":\"getCapabilities\",\"params\":{\"unknown\":true}}\n"
    ).as_bytes()).await.unwrap();
    drop(input);
    let output = tokio::time::timeout(Duration::from_secs(3), child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let responses: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), 3);
    assert_eq!(
        responses.iter().find(|r| r["id"] == "cancel").unwrap()["result"]["status"],
        "released"
    );
    assert_eq!(
        responses.iter().find(|r| r["id"] == "caps").unwrap()["result"]["platform"],
        "windows"
    );
    assert_eq!(
        responses.iter().find(|r| r["id"] == "bad").unwrap()["error"]["code"],
        "invalid-request"
    );
}

#[tokio::test(start_paused = true)]
async fn expired_region_lease_waits_for_native_release() {
    let (mut input, host_input) = tokio::io::duplex(4096);
    let (host_output, output) = tokio::io::duplex(4096);
    let engine = Arc::new(ControlledEngine::new());
    let task = tokio::spawn(lumiere_windows_host::serve(
        host_input,
        host_output,
        engine.clone(),
    ));
    input.write_all(b"{\"version\":6,\"id\":\"lease\",\"method\":\"captureRegion\",\"params\":{\"delivery\":\"clipboard\"}}\n").await.unwrap();
    while !engine.started.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    tokio::time::advance(Duration::from_secs(
        lumiere_capture_contract::REGION_LEASE_SECONDS,
    ))
    .await;
    let mut reader = BufReader::new(output);
    let response = line(&mut reader).await;
    assert_eq!(response["result"]["status"], "cancelled");
    assert!(engine.released.load(Ordering::Acquire));
    input.shutdown().await.unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn broken_output_pipe_cancels_and_releases_before_exit() {
    let (mut input, host_input) = tokio::io::duplex(4096);
    let (host_output, output) = tokio::io::duplex(4096);
    drop(output);
    let engine = Arc::new(ControlledEngine::new());
    let task = tokio::spawn(lumiere_windows_host::serve(
        host_input,
        host_output,
        engine.clone(),
    ));
    input.write_all(concat!(
        "{\"version\":6,\"id\":\"capture\",\"method\":\"captureRegion\",\"params\":{\"delivery\":\"clipboard\"}}\n",
        "{\"version\":5,\"id\":\"caps\",\"method\":\"getCapabilities\",\"params\":{}}\n"
    ).as_bytes()).await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(3), task)
        .await
        .unwrap()
        .unwrap();
    assert!(result.is_err());
    assert!(engine.released.load(Ordering::Acquire));
}
