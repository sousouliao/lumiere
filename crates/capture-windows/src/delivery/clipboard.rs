//! Immediate PNG + sRGB DIBV5 clipboard ownership, with no delayed-render callbacks.
use windows::{
    Win32::{
        Foundation::{GlobalFree, HANDLE, HGLOBAL, HWND},
        Graphics::Gdi::{BI_BITFIELDS, BITMAPV5HEADER, LCS_GM_IMAGES},
        System::{DataExchange::*, Memory::*},
        UI::WindowsAndMessaging::*,
    },
    core::{Error, Result, w},
};

const DIBV5: u32 = 17;
const SRGB: u32 = 0x73524742;

struct Window(HWND);
impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: Window was created and remains owned by this native thread.
        unsafe {
            let _ = DestroyWindow(self.0);
        }
    }
}
struct Open;
impl Drop for Open {
    fn drop(&mut self) {
        // SAFETY: This guard owns a successful OpenClipboard on this thread.
        unsafe {
            let _ = CloseClipboard();
        }
    }
}
struct Memory(Option<HGLOBAL>);
impl Memory {
    fn copy(bytes: &[u8]) -> Result<Self> {
        // SAFETY: Movable allocation is exclusively owned; copied bytes fit the requested size.
        unsafe {
            let handle = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes.len())?;
            let memory = Self(Some(handle));
            let pointer = GlobalLock(handle);
            if pointer.is_null() {
                return Err(Error::from_thread());
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), pointer.cast::<u8>(), bytes.len());
            let _ = GlobalUnlock(handle);
            Ok(memory)
        }
    }
    fn transfer(mut self, format: u32) -> Result<()> {
        let handle = self.0.expect("owned global memory");
        // SAFETY: Clipboard is open; handle is movable, populated and unlocked. Windows owns it on success.
        unsafe {
            SetClipboardData(format, Some(HANDLE(handle.0)))?;
        }
        self.0 = None;
        Ok(())
    }
}
impl Drop for Memory {
    fn drop(&mut self) {
        if let Some(handle) = self.0 {
            // SAFETY: Untransferred global memory remains exclusively owned by this guard.
            unsafe {
                let _ = GlobalFree(Some(handle));
            }
        }
    }
}
fn png_format() -> Result<u32> {
    // SAFETY: Static nul-terminated format name; Windows owns format registration.
    let format = unsafe { RegisterClipboardFormatW(w!("PNG")) };
    if format == 0 {
        Err(Error::from_thread())
    } else {
        Ok(format)
    }
}
fn open(owner: Option<HWND>) -> Result<Open> {
    for attempt in 0..10 {
        // SAFETY: Optional owner belongs to this thread; each successful call is balanced by Open::drop.
        match unsafe { OpenClipboard(owner) } {
            Ok(()) => return Ok(Open),
            Err(error) if attempt == 9 => return Err(error),
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
    unreachable!()
}
pub(super) fn write(png: &[u8]) -> std::result::Result<(), String> {
    let dib = dib_from_png(png)?;
    let write = || -> Result<()> {
        let png_memory = Memory::copy(png)?;
        let dib_memory = Memory::copy(&dib)?;
        let format = png_format()?;
        // SAFETY: Built-in STATIC class, invisible message-only window; no external callback pointer.
        let window = Window(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                w!("Lumiere clipboard"),
                WINDOW_STYLE::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            )?
        });
        let _open = open(Some(window.0))?;
        // SAFETY: Non-null owner and clipboard lock are held; subsequent data is immediately rendered.
        unsafe {
            EmptyClipboard()?;
        }
        png_memory.transfer(format)?;
        dib_memory.transfer(DIBV5)?;
        Ok(())
    };
    write().map_err(|error| error.to_string())
}
fn dib_from_png(png: &[u8]) -> std::result::Result<Vec<u8>, String> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .map_err(|e| e.to_string())?;
    let mut pixels = vec![0; reader.output_buffer_size().ok_or("PNG is too large")?];
    let info = reader.next_frame(&mut pixels).map_err(|e| e.to_string())?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err("Clipboard requires RGBA8 sRGB pixels".into());
    }
    pixels.truncate(info.buffer_size());
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    let header = BITMAPV5HEADER {
        bV5Size: size_of::<BITMAPV5HEADER>() as u32,
        bV5Width: i32::try_from(info.width).map_err(|e| e.to_string())?,
        bV5Height: -i32::try_from(info.height).map_err(|e| e.to_string())?,
        bV5Planes: 1,
        bV5BitCount: 32,
        bV5Compression: BI_BITFIELDS,
        bV5SizeImage: u32::try_from(pixels.len()).map_err(|e| e.to_string())?,
        bV5RedMask: 0x00ff0000,
        bV5GreenMask: 0x0000ff00,
        bV5BlueMask: 0x000000ff,
        bV5AlphaMask: 0xff000000,
        bV5CSType: SRGB,
        bV5Intent: LCS_GM_IMAGES as u32,
        ..Default::default()
    };
    // SAFETY: Native POD header is fully initialized; its bytes are copied into owned storage.
    let bytes = unsafe {
        std::slice::from_raw_parts(
            (&header as *const BITMAPV5HEADER).cast::<u8>(),
            size_of::<BITMAPV5HEADER>(),
        )
    };
    let mut dib = Vec::with_capacity(bytes.len() + pixels.len());
    dib.extend_from_slice(bytes);
    dib.extend_from_slice(&pixels);
    Ok(dib)
}

#[cfg(test)]
pub(crate) fn read_png(length: usize) -> Vec<u8> {
    let _open = open(None).unwrap();
    // SAFETY: Clipboard is locked; borrowed global memory is locked, copied and unlocked before closing.
    unsafe {
        let handle = HGLOBAL(GetClipboardData(png_format().unwrap()).unwrap().0);
        assert!(GlobalSize(handle) >= length);
        let pointer = GlobalLock(handle);
        assert!(!pointer.is_null());
        let bytes = std::slice::from_raw_parts(pointer.cast::<u8>(), length).to_vec();
        let _ = GlobalUnlock(handle);
        assert!(IsClipboardFormatAvailable(DIBV5).is_ok());
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dib_is_top_down_srgb_and_preserves_rgba_channels() {
        let png = crate::delivery::encode_png(2, 1, &[10, 20, 30, 255, 200, 100, 50, 128]).unwrap();
        let dib = dib_from_png(&png).unwrap();
        assert_eq!(u32::from_le_bytes(dib[0..4].try_into().unwrap()), 124);
        assert_eq!(i32::from_le_bytes(dib[8..12].try_into().unwrap()), -1);
        assert_eq!(u32::from_le_bytes(dib[56..60].try_into().unwrap()), SRGB);
        assert_eq!(&dib[124..], &[30, 20, 10, 255, 50, 100, 200, 128]);
    }
}
