use crate::{
    Cancellation,
    graphics::{Device, DisplayColor, display_color},
    interop::cursor_monitor,
};
use std::time::{Duration, Instant};
use windows::{
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
impl Drop for Frame {
    fn drop(&mut self) {
        let _ = self.0.Close();
    }
}

pub(crate) fn freeze(device: &Device, cancel: &Cancellation) -> Result<Option<FrozenFrame>> {
    let monitor = cursor_monitor()?;
    let color = display_color(&monitor)?;
    let interop: IGraphicsCaptureItemInterop =
        factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()?;
    // SAFETY: OS-owned monitor handle is valid for this capture; interface projection owns the result.
    let item: GraphicsCaptureItem = unsafe { interop.CreateForMonitor(monitor.handle)? };
    let size = item.Size()?;
    if size.Width <= 0 || size.Height <= 0 {
        return Err(Error::new(E_FAIL, "Empty capture target"));
    }
    // SAFETY: Native device is owned and valid; keep the apartment-bound wrapper local.
    let winrt: IDirect3DDevice = unsafe {
        CreateDirect3D11DeviceFromDXGIDevice(&device.native.cast::<IDXGIDevice>()?)?.cast()?
    };
    let pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &winrt,
        DirectXPixelFormat::R16G16B16A16Float,
        2,
        size,
    )?;
    let mut resources = Session {
        session: None,
        pool,
    };
    let session = resources.pool.CreateCaptureSession(&item)?;
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
        let access: IDirect3DDxgiInterfaceAccess = frame.0.Surface()?.cast()?;
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
        }));
    }
}
