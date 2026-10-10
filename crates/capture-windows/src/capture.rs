use crate::{
    Cancellation,
    graphics::{Device, DisplayColor, display_color},
    interop::{Monitor, cursor_monitor},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windows::{
    Foundation::TypedEventHandler,
    Graphics::{
        Capture::*,
        DirectX::{Direct3D11::IDirect3DDevice, DirectXPixelFormat},
    },
    Win32::{
        Foundation::{E_FAIL, S_OK},
        Graphics::{Direct3D11::ID3D11Texture2D, Dxgi::IDXGIDevice},
        System::WinRT::{
            Direct3D11::{CreateDirect3D11DeviceFromDXGIDevice, IDirect3DDxgiInterfaceAccess},
            Graphics::Capture::IGraphicsCaptureItemInterop,
        },
    },
    core::{Error, Interface, Result, factory},
};

pub(crate) struct FrozenFrame {
    pub texture: ID3D11Texture2D,
    pub width: u32,
    pub height: u32,
    pub color: DisplayColor,
    pub monitor: crate::interop::Monitor,
}

// Retain target descriptors only. Sessions, pools and frames remain capture-scoped.
#[derive(Default)]
pub(crate) struct Targets(Vec<Target>);
struct Target {
    monitor: Monitor,
    item: GraphicsCaptureItem,
    closed: Arc<AtomicBool>,
    closed_token: i64,
}
impl Target {
    fn matches(&self, monitor: &Monitor) -> bool {
        self.monitor.handle == monitor.handle
            && self.monitor.name == monitor.name
            && self.monitor.bounds == monitor.bounds
    }
}
impl Drop for Target {
    fn drop(&mut self) {
        let _ = self.item.RemoveClosed(self.closed_token);
    }
}
impl Targets {
    fn item(&mut self, monitor: &Monitor) -> Result<&GraphicsCaptureItem> {
        self.0.retain(|target| {
            !target.closed.load(Ordering::Acquire)
                && (target.monitor.handle != monitor.handle || target.matches(monitor))
        });
        let index = if let Some(index) = self.0.iter().position(|target| target.matches(monitor)) {
            index
        } else {
            let interop: IGraphicsCaptureItemInterop =
                factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
            // SAFETY: The current monitor is OS-owned and resolved on this capture worker.
            let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(monitor.handle)? };
            let closed = Arc::new(AtomicBool::new(false));
            let signal = closed.clone();
            let closed_token = item.Closed(&TypedEventHandler::new(move |_, _| {
                signal.store(true, Ordering::Release);
                Ok(())
            }))?;
            self.0.push(Target {
                monitor: Monitor {
                    handle: monitor.handle,
                    name: monitor.name.clone(),
                    bounds: monitor.bounds,
                },
                item,
                closed,
                closed_token,
            });
            self.0.len() - 1
        };
        Ok(&self.0[index].item)
    }
    pub fn clear(&mut self) {
        self.0.clear();
    }
}
struct Session {
    session: Option<GraphicsCaptureSession>,
    pool: Direct3D11CaptureFramePool,
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(session) = &self.session {
            let _ = session.Close();
        }
        let _ = self.pool.Close();
    }
}
struct Frame(Direct3D11CaptureFrame);
struct DirectDevice(IDirect3DDevice);
struct Surface(windows::Graphics::DirectX::Direct3D11::IDirect3DSurface);
impl Drop for Surface {
    fn drop(&mut self) {
        let _ = self.0.Close();
    }
}
impl Drop for DirectDevice {
    fn drop(&mut self) {
        let _ = self.0.Close();
    }
}
impl Drop for Frame {
    fn drop(&mut self) {
        let _ = self.0.Close();
    }
}

pub(crate) fn freeze(
    device: &Device,
    targets: &mut Targets,
    cancel: &Cancellation,
) -> Result<Option<FrozenFrame>> {
    let monitor = cursor_monitor()?;
    let color = display_color(&monitor)?;
    let item = targets.item(&monitor)?;
    let size = item.Size()?;
    if size.Width <= 0 || size.Height <= 0 {
        return Err(Error::new(E_FAIL, "Empty capture target"));
    }
    // SAFETY: Native device is owned and valid; keep the apartment-bound wrapper local.
    let winrt = DirectDevice(unsafe {
        CreateDirect3D11DeviceFromDXGIDevice(&device.native.cast::<IDXGIDevice>()?)?.cast()?
    });
    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &winrt.0,
        DirectXPixelFormat::R16G16B16A16Float,
        2,
        size,
    )?;
    let mut resources = Session {
        session: None,
        pool,
    };
    let session = resources.pool.CreateCaptureSession(item)?;
    // Best effort, same unpackaged-app border policy as the baseline.
    let _ = session.SetIsBorderRequired(false);
    resources.session = Some(session);
    resources
        .session
        .as_ref()
        .expect("session assigned")
        .StartCapture()?;
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if cancel.is_cancelled() {
            return Ok(None);
        }
        if Instant::now() >= deadline {
            return Err(Error::new(E_FAIL, "Timed out waiting for a WGC frame"));
        }
        let frame = match resources.pool.TryGetNextFrame() {
            Ok(frame) => Frame(frame),
            // windows-core 0.62 maps a successful null interface to Error::empty (S_OK).
            Err(error) if error.code() == S_OK => {
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            Err(error) => return Err(error),
        };
        let content = frame.0.ContentSize()?;
        if content.Width != size.Width || content.Height != size.Height {
            return Err(Error::new(
                E_FAIL,
                "Capture target changed size; retry capture",
            ));
        }
        let surface = Surface(frame.0.Surface()?);
        let access: IDirect3DDxgiInterfaceAccess = surface.0.cast()?;
        // SAFETY: Surface access returns its owned D3D11 texture; copy has identical dimensions/format.
        let texture = unsafe {
            let source: ID3D11Texture2D = access.GetInterface()?;
            let mut desc = windows::Win32::Graphics::Direct3D11::D3D11_TEXTURE2D_DESC::default();
            source.GetDesc(&mut desc);
            if desc.Width < size.Width as u32
                || desc.Height < size.Height as u32
                || desc.Format
                    != windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R16G16B16A16_FLOAT
            {
                return Err(Error::new(
                    E_FAIL,
                    "Unexpected WGC frame dimensions or format",
                ));
            }
            let texture = device.texture(size.Width as u32, size.Height as u32, false)?;
            let region = windows::Win32::Graphics::Direct3D11::D3D11_BOX {
                left: 0,
                top: 0,
                front: 0,
                right: size.Width as u32,
                bottom: size.Height as u32,
                back: 1,
            };
            device
                .context
                .CopySubresourceRegion(&texture, 0, 0, 0, 0, &source, 0, Some(&region));
            texture
        };
        return Ok(Some(FrozenFrame {
            texture,
            width: size.Width as u32,
            height: size.Height as u32,
            color,
            monitor,
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interop::{Apartment, DpiScope};
    use windows::Win32::System::WinRT::RO_INIT_MULTITHREADED;

    #[test]
    #[ignore = "Creates real monitor capture descriptors; run explicitly"]
    fn target_cache_reuses_and_invalidates_descriptors() {
        let _apartment = Apartment::initialize(RO_INIT_MULTITHREADED).unwrap();
        let _dpi = DpiScope::physical_coordinates().unwrap();
        let mut monitor = cursor_monitor().unwrap();
        let mut targets = Targets::default();
        let first = targets.item(&monitor).unwrap().clone();
        assert_eq!(&first, targets.item(&monitor).unwrap());
        assert_eq!(targets.0.len(), 1);

        // Exercise the invalidation signal used by the Closed callback.
        targets.0[0].closed.store(true, Ordering::Release);
        let reopened = targets.item(&monitor).unwrap().clone();
        assert_ne!(first, reopened);
        assert_eq!(targets.0.len(), 1);

        monitor.bounds.right += 1;
        assert_ne!(&reopened, targets.item(&monitor).unwrap());
        assert_eq!(
            targets.0.len(),
            1,
            "Changed geometry replaces the old descriptor"
        );
        targets.clear();
        assert!(targets.0.is_empty());
    }
}
