//! Native engine boundary. This crate never depends on Tauri or JSONL transports.
use lumiere_capture_contract::{Capabilities, CaptureMode, CaptureOutcome, CaptureParams, Failure};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

mod capture;
mod delivery;
mod graphics;
mod interop;

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

pub struct WindowsEngine {
    send: Option<mpsc::Sender<CaptureJob>>,
    worker: Option<std::thread::JoinHandle<()>>,
    startup_failure: Option<Failure>,
}
struct CaptureJob {
    params: CaptureParams,
    cancel: Cancellation,
    reply: mpsc::SyncSender<CaptureOutcome>,
}
impl Default for WindowsEngine {
    fn default() -> Self {
        let (send, receive) = mpsc::channel::<CaptureJob>();
        let (ready, started) = mpsc::sync_channel(1);
        let worker = std::thread::Builder::new()
            .name("Lumiere capture".into())
            .spawn(move || {
                let apartment = interop::Apartment::initialize(
                    windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
                );
                let _apartment = match apartment {
                    Ok(guard) => {
                        let _ = ready.send(None);
                        guard
                    }
                    Err(error) => {
                        let _ = ready.send(Some(Failure::capture(error.to_string())));
                        return;
                    }
                };
                let mut device = None;
                while let Ok(job) = receive.recv() {
                    let outcome = match Self::capture_display(&mut device, job.params, job.cancel) {
                        Ok(result) => result,
                        Err(error) => CaptureOutcome::Failed {
                            failure: Failure::capture(error.to_string()),
                        },
                    };
                    let _ = job.reply.send(outcome);
                }
                // Device is released on its owning thread, before the MTA apartment.
                drop(device);
            })
            .expect("create native capture worker");
        let startup_failure = started
            .recv()
            .unwrap_or_else(|_| Some(Failure::capture("Native worker failed to initialize")));
        Self {
            send: Some(send),
            worker: Some(worker),
            startup_failure,
        }
    }
}
impl Drop for WindowsEngine {
    fn drop(&mut self) {
        drop(self.send.take());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl CaptureEngine for WindowsEngine {
    fn capabilities(&self, version: u8) -> Capabilities {
        if let Some(reason) = &self.startup_failure {
            return Capabilities::unavailable(version, reason.clone());
        }
        let probe = || -> windows::core::Result<&'static str> {
            let _apartment = interop::Apartment::initialize(
                windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
            )?;
            if !windows::Graphics::Capture::GraphicsCaptureSession::IsSupported()? {
                return Err(windows::core::Error::new(
                    windows::Win32::Foundation::E_FAIL,
                    "Windows Graphics Capture is not supported",
                ));
            }
            let _device = graphics::Device::create()?;
            let hdr = interop::cursor_monitor().and_then(|m| graphics::display_color(&m));
            Ok(match hdr {
                Ok(color) => match color.range {
                    lumiere_capture_contract::DynamicRange::Hdr => "supported",
                    lumiere_capture_contract::DynamicRange::Sdr => "unavailable",
                },
                Err(_) => "unvalidated",
            })
        };
        match probe() {
            Ok(hdr_capture) => Capabilities {
                contract_version: version,
                platform: "windows",
                host_status: "available",
                capture_modes: vec![CaptureMode::Display],
                delivery_targets: vec![
                    lumiere_capture_contract::DeliveryTarget::Clipboard,
                    lumiere_capture_contract::DeliveryTarget::Folder,
                ],
                hdr_capture,
                output_profiles: vec!["srgb-visual-match"],
                unavailable_reason: None,
            },
            Err(error) => Capabilities::unavailable(version, Failure::capture(error.to_string())),
        }
    }
    fn capture(
        &self,
        mode: CaptureMode,
        params: CaptureParams,
        cancel: Cancellation,
    ) -> CaptureOutcome {
        if cancel.is_cancelled() {
            return CaptureOutcome::Cancelled;
        }
        if mode != CaptureMode::Display {
            return CaptureOutcome::Failed {
                failure: Failure::capture("Native Region migration is not available yet."),
            };
        }
        if let Err(failure) = params.validate() {
            return CaptureOutcome::Failed { failure };
        }
        let (reply, result) = mpsc::sync_channel(1);
        if self.send.as_ref().is_none_or(|send| {
            send.send(CaptureJob {
                params,
                cancel,
                reply,
            })
            .is_err()
        }) {
            return CaptureOutcome::Failed {
                failure: Failure::capture("Native capture worker is unavailable"),
            };
        }
        result.recv().unwrap_or_else(|_| CaptureOutcome::Failed {
            failure: Failure::unexpected("Native capture worker failed"),
        })
    }
}

impl WindowsEngine {
    fn capture_display(
        cache: &mut Option<graphics::Device>,
        params: CaptureParams,
        cancel: Cancellation,
    ) -> windows::core::Result<CaptureOutcome> {
        if cache.is_none() {
            *cache = Some(graphics::Device::create()?);
        }
        let device = cache.as_ref().expect("device initialized");
        let result = (|| {
            let Some(frame) = capture::freeze(device, &cancel)? else {
                return Ok(CaptureOutcome::Cancelled);
            };
            if cancel.is_cancelled() {
                return Ok(CaptureOutcome::Cancelled);
            }
            let source = device.readback(&frame.texture, frame.width, frame.height)?;
            let pixels = graphics::rgba16f_to_rgba8(&source, frame.width, frame.color.scale);
            if cancel.is_cancelled() {
                return Ok(CaptureOutcome::Cancelled);
            }
            let png = delivery::encode_png(frame.width, frame.height, &pixels).map_err(|e| {
                windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string())
            })?;
            if cancel.is_cancelled() {
                return Ok(CaptureOutcome::Cancelled);
            }
            let deliveries = delivery::deliver(&params, &png, &cancel);
            if cancel.is_cancelled() {
                return Ok(CaptureOutcome::Cancelled);
            }
            Ok(CaptureOutcome::Completed {
                source_dynamic_range: frame.color.range,
                output_profile: "srgb-visual-match",
                deliveries,
            })
        })();
        // Retry with fresh native resources after device removal or any acquisition/readback failure.
        if result.is_err() {
            *cache = None;
        }
        result
    }
}

#[cfg(test)]
mod native_tests {
    use super::*;
    use lumiere_capture_contract::{Delivery, DeliveryOutcome};

    #[test]
    #[ignore = "Captures the real desktop and writes the Windows clipboard; run explicitly"]
    fn display_both_deliveries_share_identical_png_on_this_machine() {
        let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../artifacts/windows/rust-display");
        std::fs::create_dir_all(&folder).unwrap();
        let folder = folder.canonicalize().unwrap();
        let started = std::time::Instant::now();
        let engine = WindowsEngine::default();
        let outcome = engine.capture(
            CaptureMode::Display,
            CaptureParams {
                delivery: Delivery::Both,
                save_directory: Some(folder.to_string_lossy().into_owned()),
            },
            Cancellation::default(),
        );
        let CaptureOutcome::Completed {
            deliveries,
            source_dynamic_range,
            ..
        } = outcome
        else {
            panic!("Capture failed: {outcome:?}");
        };
        assert_eq!(deliveries.len(), 2);
        assert!(matches!(
            deliveries[0].outcome,
            DeliveryOutcome::Success { file_path: None }
        ));
        let DeliveryOutcome::Success {
            file_path: Some(path),
        } = &deliveries[1].outcome
        else {
            panic!("Folder delivery failed: {deliveries:?}");
        };
        let file = std::fs::read(path).unwrap();
        let clipboard = delivery::read_clipboard_png(file.len());
        assert_eq!(
            file, clipboard,
            "Clipboard and folder must share the exact encoded artifact"
        );
        let reader = png::Decoder::new(std::io::Cursor::new(&file))
            .read_info()
            .unwrap();
        assert!(reader.info().width > 0 && reader.info().height > 0);
        println!(
            "range={source_dynamic_range:?}, dimensions={}x{}, pngBytes={}, elapsedMs={}, artifact={path}",
            reader.info().width,
            reader.info().height,
            file.len(),
            started.elapsed().as_millis()
        );

        let blocked = folder.join("blocked-folder");
        std::fs::write(&blocked, b"folder blocker").unwrap();
        let results = delivery::deliver(
            &CaptureParams {
                delivery: Delivery::Both,
                save_directory: Some(blocked.to_string_lossy().into_owned()),
            },
            &file,
            &Cancellation::default(),
        );
        std::fs::remove_file(blocked).unwrap();
        assert!(matches!(
            results[0].outcome,
            DeliveryOutcome::Success { .. }
        ));
        assert!(matches!(results[1].outcome, DeliveryOutcome::Failed { .. }));
        assert_eq!(
            file,
            delivery::read_clipboard_png(file.len()),
            "Failed folder delivery must preserve successful clipboard output"
        );
    }
}
