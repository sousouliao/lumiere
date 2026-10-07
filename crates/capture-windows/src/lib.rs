//! Native engine boundary. This crate never depends on Tauri or JSONL transports.
use lumiere_capture_contract::{Capabilities, CaptureMode, CaptureOutcome, CaptureParams, Failure};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub struct Cancellation(Arc<AtomicBool>);

impl Cancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

/// A blocking capture operation owns its native thread/resources until it returns.
/// The transport reserves a request before calling into this interface.
pub trait CaptureEngine: Send + Sync + 'static {
    fn capabilities(&self, version: u8) -> Capabilities;
    fn capture(
        &self,
        mode: CaptureMode,
        params: CaptureParams,
        cancel: Cancellation,
    ) -> CaptureOutcome;
}

pub struct WindowsEngine;

impl CaptureEngine for WindowsEngine {
    fn capabilities(&self, version: u8) -> Capabilities {
        Capabilities::unavailable(
            version,
            Failure::capture("Rust native capture migration is not available yet."),
        )
    }
    fn capture(
        &self,
        _mode: CaptureMode,
        _params: CaptureParams,
        cancel: Cancellation,
    ) -> CaptureOutcome {
        if cancel.is_cancelled() {
            CaptureOutcome::Cancelled
        } else {
            CaptureOutcome::Failed {
                failure: Failure::capture("Rust native capture migration is not available yet."),
            }
        }
    }
}
