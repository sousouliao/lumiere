//! The shell supervises a separate native process; it never owns capture resources.
use lumiere_capture_contract::HostResult;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::Path,
    process::Stdio,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, Command},
    sync::oneshot,
    task::JoinHandle,
};

type Reply = Result<Value, String>;
type Pending = Arc<Mutex<HashMap<String, (u8, oneshot::Sender<Reply>)>>>;

pub struct Host {
    input: tokio::sync::Mutex<Option<ChildStdin>>,
    child: tokio::sync::Mutex<Option<Child>>,
    pending: Pending,
    reader: tokio::sync::Mutex<Option<JoinHandle<()>>>,
    next_id: AtomicU64,
    alive: Arc<AtomicBool>,
}

impl Host {
    pub fn launch(executable: &Path) -> Result<Self, String> {
        let mut command = Command::new(executable);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // CREATE_NO_WINDOW; native Region owns its HWND.
        let mut child = command.spawn().map_err(|error| error.to_string())?;
        let input = child.stdin.take().ok_or("Host stdin unavailable")?;
        let output = child.stdout.take().ok_or("Host stdout unavailable")?;
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let responses = pending.clone();
        let alive = Arc::new(AtomicBool::new(true));
        let reader_alive = alive.clone();
        let reader = tokio::spawn(async move {
            let mut lines = BufReader::new(output).lines();
            let reason = loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        if let Err(error) = route_response(&responses, &line) {
                            break error;
                        }
                    }
                    Ok(None) => break "Native capture host exited".to_owned(),
                    Err(error) => break error.to_string(),
                }
            };
            reader_alive.store(false, Ordering::Release);
            fail_pending(&responses, &reason);
        });
        Ok(Self {
            input: tokio::sync::Mutex::new(Some(input)),
            child: tokio::sync::Mutex::new(Some(child)),
            pending,
            reader: tokio::sync::Mutex::new(Some(reader)),
            next_id: AtomicU64::new(1),
            alive,
        })
    }

    pub fn request_id(&self) -> String {
        format!("shell-{}", self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
    #[cfg(test)]
    pub async fn process_id(&self) -> Option<u32> {
        self.child.lock().await.as_ref().and_then(Child::id)
    }

    pub async fn request(
        &self,
        id: String,
        version: u8,
        method: &str,
        params: Value,
    ) -> Result<HostResult, String> {
        if !self.is_alive() {
            return Err("Native capture host is stopped".into());
        }
        let (sender, receiver) = oneshot::channel();
        {
            let mut pending = self
                .pending
                .lock()
                .map_err(|_| "Host response state unavailable")?;
            if pending.contains_key(&id) {
                return Err("Duplicate Host request ID".into());
            }
            pending.insert(id.clone(), (version, sender));
        }
        let mut line =
            serde_json::to_vec(&json!({"version":version,"id":id,"method":method,"params":params}))
                .map_err(|error| error.to_string())?;
        line.push(b'\n');
        {
            let mut input = self.input.lock().await;
            let input = input.as_mut().ok_or("Native capture host is stopped")?;
            if let Err(error) = async {
                input.write_all(&line).await?;
                input.flush().await
            }
            .await
            {
                self.alive.store(false, Ordering::Release);
                fail_pending(&self.pending, &error.to_string());
                return Err(error.to_string());
            }
        }
        let limit = if method == "captureRegion" { 70 } else { 15 };
        match tokio::time::timeout(Duration::from_secs(limit), receiver).await {
            Ok(Ok(reply)) => match reply {
                Ok(value) => match HostResult::decode(method, version, &params, value) {
                    Ok(result) => Ok(result),
                    Err(error) => {
                        self.shutdown().await;
                        Err(error.into())
                    }
                },
                Err(error) => Err(error),
            },
            Ok(Err(_)) => Err("Native capture host disconnected".into()),
            Err(_) => {
                self.shutdown().await;
                Err("Native capture host timed out".into())
            }
        }
    }

    /// EOF asks the Host to cancel/join active native work before exit. The bounded
    /// fallback kills a broken process; kill_on_drop also covers shell failure.
    pub async fn shutdown(&self) {
        if let Err(error) = self.shutdown_checked().await {
            eprintln!(
                "{}",
                serde_json::json!({"level":"warn","event":"host-shutdown-failed","error":error})
            );
        }
    }
    pub async fn shutdown_checked(&self) -> Result<(), String> {
        self.alive.store(false, Ordering::Release);
        self.input.lock().await.take();
        let mut slot = self.child.lock().await;
        if let Some(mut child) = slot.take() {
            let result = match tokio::time::timeout(Duration::from_secs(12), child.wait()).await {
                Ok(Ok(_)) => Ok(()),
                _ => match child.kill().await {
                    Ok(()) => child
                        .wait()
                        .await
                        .map(|_| ())
                        .map_err(|error| error.to_string()),
                    Err(error) => Err(error.to_string()),
                },
            };
            if let Err(error) = result {
                *slot = Some(child);
                return Err(error);
            }
        }
        drop(slot);
        if let Some(reader) = self.reader.lock().await.take() {
            reader.abort();
            let _ = reader.await;
        }
        fail_pending(&self.pending, "Native capture host stopped");
        Ok(())
    }
}

fn fail_pending(pending: &Pending, reason: &str) {
    if let Ok(mut pending) = pending.lock() {
        for (_, (_, reply)) in pending.drain() {
            let _ = reply.send(Err(reason.to_owned()));
        }
    }
}

fn route_response(pending: &Pending, line: &str) -> Result<(), String> {
    let response: Value = serde_json::from_str(line).map_err(|_| "Invalid Host JSON response")?;
    let object = response.as_object().ok_or("Invalid Host response")?;
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .ok_or("Missing Host response ID")?;
    let version = object
        .get("version")
        .and_then(Value::as_u64)
        .ok_or("Missing Host response version")?;
    if object.len() != 3 || object.contains_key("result") == object.contains_key("error") {
        return Err("Invalid Host response envelope".into());
    }
    let mut pending = pending
        .lock()
        .map_err(|_| "Host response state unavailable")?;
    let (expected, sender) = pending.remove(id).ok_or("Unknown Host response ID")?;
    if version != u64::from(expected) {
        let _ = sender.send(Err("Host response version mismatch".into()));
        return Err("Host response version mismatch".into());
    }
    let result = if let Some(error) = object.get("error") {
        match serde_json::from_value::<lumiere_capture_contract::Failure>(error.clone()) {
            Ok(error) => Err(error.message),
            Err(_) => Err("Invalid Host error response".into()),
        }
    } else {
        Ok(object["result"].clone())
    };
    let _ = sender.send(result);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires built native Host and Windows capability APIs"]
    async fn supervises_real_child_disconnect_and_clean_replacement() {
        let executable = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target/debug/lumiere-windows-host.exe");
        let host = Host::launch(&executable).unwrap();
        assert!(matches!(
            host.request(host.request_id(), 5, "getCapabilities", json!({}))
                .await
                .unwrap(),
            HostResult::Capabilities(_)
        ));
        host.child
            .lock()
            .await
            .as_mut()
            .unwrap()
            .kill()
            .await
            .unwrap();
        let response = tokio::time::timeout(
            Duration::from_secs(2),
            host.request(host.request_id(), 5, "getCapabilities", json!({})),
        )
        .await
        .unwrap();
        assert!(response.is_err());
        host.shutdown().await;
        assert!(host.process_id().await.is_none());
        let replacement = Host::launch(&executable).unwrap();
        assert!(matches!(
            replacement
                .request(replacement.request_id(), 5, "getCapabilities", json!({}))
                .await
                .unwrap(),
            HostResult::Capabilities(_)
        ));
        replacement.shutdown().await;
        assert!(replacement.process_id().await.is_none());
    }

    #[tokio::test]
    async fn matches_response_and_rejects_protocol_drift() {
        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let (tx, rx) = oneshot::channel();
        pending.lock().unwrap().insert("one".into(), (6, tx));
        route_response(
            &pending,
            r#"{"version":6,"id":"one","result":{"status":"released"}}"#,
        )
        .unwrap();
        assert_eq!(rx.await.unwrap().unwrap()["status"], "released");
        assert!(route_response(&pending, r#"{"version":6,"id":"unknown","result":{}}"#).is_err());
        let (tx, rx) = oneshot::channel();
        pending.lock().unwrap().insert("two".into(), (6, tx));
        assert!(route_response(&pending, r#"{"version":5,"id":"two","result":{}}"#).is_err());
        assert!(rx.await.unwrap().is_err());
    }
}
