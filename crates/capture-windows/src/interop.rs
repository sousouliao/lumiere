//! Small native boundaries; apartment guards never cross threads.
use std::{marker::PhantomData, rc::Rc};
use windows::{
    Win32::{Foundation::POINT, Graphics::Gdi::*, System::WinRT::*},
    core::{Error, Result},
};

pub(crate) struct Apartment(PhantomData<Rc<()>>);
impl Apartment {
    pub fn initialize(kind: RO_INIT_TYPE) -> Result<Self> {
        // SAFETY: Initialization and balanced uninitialization happen on this thread.
        unsafe { RoInitialize(kind)? };
        Ok(Self(PhantomData))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        // SAFETY: A successfully initialized, non-Send apartment guard owns this call.
        unsafe { RoUninitialize() };
    }
}

pub(crate) struct Monitor {
    pub handle: HMONITOR,
    pub name: String,
    pub bounds: windows::Win32::Foundation::RECT,
}
pub(crate) fn cursor_monitor() -> Result<Monitor> {
    let mut point = POINT::default();
    // SAFETY: Writable stack point; returned monitor handles are borrowed from Windows.
    unsafe {
        windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point)?;
        let handle = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if !GetMonitorInfoW(handle, &mut info as *mut _ as *mut MONITORINFO).as_bool() {
            return Err(Error::from_thread());
        }
        Ok(Monitor {
            handle,
            name: utf16(&info.szDevice),
            bounds: info.monitorInfo.rcMonitor,
        })
    }
}

pub(crate) struct DpiScope(windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT);
impl DpiScope {
    pub fn physical_coordinates() -> Result<Self> {
        // SAFETY: Thread-local DPI context is restored by a non-Send guard on this worker.
        let previous = unsafe {
            windows::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(
                windows::Win32::UI::HiDpi::DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            )
        };
        if previous.0.is_null() {
            return Err(Error::from_thread());
        }
        Ok(Self(previous))
    }
}
impl Drop for DpiScope {
    fn drop(&mut self) {
        // SAFETY: Restore this thread's saved valid DPI context.
        unsafe {
            windows::Win32::UI::HiDpi::SetThreadDpiAwarenessContext(self.0);
        }
    }
}
pub(crate) fn utf16(value: &[u16]) -> String {
    String::from_utf16_lossy(&value[..value.iter().position(|v| *v == 0).unwrap_or(value.len())])
}
