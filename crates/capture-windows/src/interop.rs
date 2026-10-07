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
        })
    }
}
pub(crate) fn utf16(value: &[u16]) -> String {
    String::from_utf16_lossy(&value[..value.iter().position(|v| *v == 0).unwrap_or(value.len())])
}
