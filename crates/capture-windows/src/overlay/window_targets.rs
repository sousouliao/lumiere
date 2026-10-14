//! Per-frozen-frame top-level window geometry, front to back. No pointer-time IPC.
use super::geometry::Rect;
use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, RECT},
        Graphics::Dwm::{DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DwmGetWindowAttribute},
        UI::WindowsAndMessaging::*,
    },
    core::{BOOL, Result},
};

pub(super) fn snapshot(excluded: HWND, monitor: RECT) -> Result<Vec<Rect>> {
    struct Snapshot {
        excluded: HWND,
        monitor: RECT,
        rectangles: Vec<Rect>,
    }
    unsafe extern "system" fn visit(window: HWND, data: LPARAM) -> BOOL {
        // SAFETY: EnumWindows is synchronous; data points to the exclusively borrowed stack snapshot.
        unsafe {
            let snapshot = &mut *(data.0 as *mut Snapshot);
            if window == snapshot.excluded
                || !IsWindowVisible(window).as_bool()
                || IsIconic(window).as_bool()
                || GetWindowLongPtrW(window, GWL_EXSTYLE) as u32 & WS_EX_TRANSPARENT.0 != 0
            {
                return true.into();
            }
            let mut cloaked = 0u32;
            let _ = DwmGetWindowAttribute(
                window,
                DWMWA_CLOAKED,
                (&mut cloaked as *mut u32).cast(),
                size_of::<u32>() as u32,
            );
            if cloaked != 0 {
                return true.into();
            }
            let mut class = [0u16; 256];
            let length = GetClassNameW(window, &mut class);
            let class = String::from_utf16_lossy(&class[..length.max(0) as usize]);
            if matches!(
                class.as_str(),
                "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
            ) {
                return true.into();
            }
            let mut bounds = RECT::default();
            if DwmGetWindowAttribute(
                window,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                (&mut bounds as *mut RECT).cast(),
                size_of::<RECT>() as u32,
            )
            .is_err()
                && GetWindowRect(window, &mut bounds).is_err()
            {
                return true.into();
            }
            if let Some(rect) = clipped(bounds, snapshot.monitor) {
                snapshot.rectangles.push(rect);
            }
            true.into()
        }
    }
    let mut snapshot = Snapshot {
        excluded,
        monitor,
        rectangles: Vec::new(),
    };
    // SAFETY: Callback and mutable snapshot storage live throughout this synchronous enumeration.
    unsafe {
        EnumWindows(
            Some(visit),
            LPARAM((&mut snapshot as *mut Snapshot) as isize),
        )?
    };
    Ok(snapshot.rectangles)
}

fn clipped(bounds: RECT, monitor: RECT) -> Option<Rect> {
    let left = bounds.left.max(monitor.left);
    let top = bounds.top.max(monitor.top);
    let right = bounds.right.min(monitor.right);
    let bottom = bounds.bottom.min(monitor.bottom);
    (right > left && bottom > top).then_some(Rect {
        left: (left - monitor.left) as f64,
        top: (top - monitor.top) as f64,
        right: (right - monitor.left) as f64,
        bottom: (bottom - monitor.top) as f64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_desktop_origin_and_cross_monitor_bounds_stay_in_physical_pixels() {
        let monitor = RECT {
            left: -1920,
            top: -200,
            right: 0,
            bottom: 880,
        };
        let rect = clipped(
            RECT {
                left: -2000,
                top: -100,
                right: 200,
                bottom: 1000,
            },
            monitor,
        )
        .unwrap();
        assert_eq!(
            rect,
            Rect {
                left: 0.0,
                top: 100.0,
                right: 1920.0,
                bottom: 1080.0
            }
        );
        assert!(
            clipped(
                RECT {
                    left: 0,
                    top: 0,
                    right: 500,
                    bottom: 500
                },
                monitor
            )
            .is_none()
        );
    }
}
