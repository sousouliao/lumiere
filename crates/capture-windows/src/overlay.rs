//! Worker-owned native selection window. No frame readback precedes interaction.
pub(crate) mod geometry;
mod presenter;
#[cfg(test)]
mod tests;
use crate::{
    Cancellation,
    capture::FrozenFrame,
    graphics::{Device, VisualMatch},
};
use geometry::{Crop, Rect};
use std::time::{Duration, Instant};
use windows::{
    Win32::{
        Foundation::{E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromPoint, ValidateRect},
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{AttachThreadInput, GetCurrentThreadId},
        },
        UI::{HiDpi::GetDpiForWindow, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::{Error, HSTRING, PCWSTR, Result, w},
};

pub(crate) enum Selection {
    Selected(Crop),
    Cancelled,
    SwitchTarget,
}
struct State {
    active: bool,
    width: f64,
    height: f64,
    scale: f64,
    pointer: (f64, f64),
    origin: Option<(f64, f64)>,
    selected: Option<Rect>,
    cancelled: bool,
    dirty: bool,
    capture: Option<bool>,
}
impl State {
    fn rect(&self) -> Option<Rect> {
        self.origin
            .and_then(|o| Rect::drag(o, self.pointer, self.width, self.height))
    }
    fn pointer(&mut self, value: LPARAM) {
        let x = (value.0 as u16 as i16) as f64;
        let y = ((value.0 >> 16) as u16 as i16) as f64;
        self.pointer = (x.clamp(0.0, self.width), y.clamp(0.0, self.height));
        self.dirty = true;
    }
}
struct Class {
    name: HSTRING,
    instance: HINSTANCE,
}
impl Drop for Class {
    fn drop(&mut self) {
        // SAFETY: Class name/instance remain valid; its owned window has already been destroyed.
        unsafe {
            let _ = UnregisterClassW(PCWSTR(self.name.as_ptr()), Some(self.instance));
        }
    }
}
struct Window(HWND);
impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: HWND and attached state still belong to this worker during teardown.
        unsafe {
            if GetCapture() == self.0 {
                let _ = ReleaseCapture();
            }
            let _ = ShowWindow(self.0, SW_HIDE);
            let _ = DestroyWindow(self.0);
        }
    }
}
unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: NCCREATE carries a pointer to the stable Box<State>, kept alive until DestroyWindow returns.
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(lparam.0 as *const CREATESTRUCTW);
            SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let pointer = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut State;
        if !pointer.is_null() {
            // No message handler invokes synchronous APIs that reenter this state borrow.
            let state = &mut *pointer;
            if !state.active
                && matches!(
                    message,
                    WM_LBUTTONDOWN
                        | WM_LBUTTONUP
                        | WM_MOUSEMOVE
                        | WM_RBUTTONDOWN
                        | WM_KEYDOWN
                        | WM_CAPTURECHANGED
                )
            {
                return LRESULT(0);
            }
            match message {
                WM_ERASEBKGND => return LRESULT(1),
                WM_PAINT => {
                    state.dirty = true;
                    let _ = ValidateRect(Some(window), None);
                    return LRESULT(0);
                }
                WM_CLOSE | WM_DESTROY | WM_DISPLAYCHANGE | WM_DPICHANGED | WM_RBUTTONDOWN => {
                    state.cancelled = true;
                    return LRESULT(0);
                }
                WM_KEYDOWN if wparam.0 == VK_ESCAPE.0 as usize => {
                    state.cancelled = true;
                    return LRESULT(0);
                }
                WM_LBUTTONDOWN => {
                    state.pointer(lparam);
                    state.origin = Some(state.pointer);
                    state.capture = Some(true);
                    return LRESULT(0);
                }
                WM_MOUSEMOVE => {
                    state.pointer(lparam);
                    return LRESULT(0);
                }
                WM_LBUTTONUP if state.origin.is_some() => {
                    state.pointer(lparam);
                    let rect = state.rect();
                    state.origin = None;
                    state.capture = Some(false);
                    if let Some(rect) = rect
                        && rect.is_valid_selection(state.scale)
                    {
                        state.selected = Some(rect);
                    }
                    return LRESULT(0);
                }
                WM_CAPTURECHANGED if state.origin.is_some() => {
                    state.cancelled = true;
                    return LRESULT(0);
                }
                _ => {}
            }
        }
        DefWindowProcW(window, message, wparam, lparam)
    }
}

pub(crate) struct Overlay {
    presenter: Option<presenter::Presenter>,
    size: (u32, u32),
    window: Window,
    state: Box<State>,
    _class: Class,
}
impl Overlay {
    fn new(frame: &FrozenFrame) -> Result<Self> {
        // SAFETY: Read this worker's module/thread identity for its private window class.
        let (instance, thread) =
            unsafe { (HINSTANCE(GetModuleHandleW(None)?.0), GetCurrentThreadId()) };
        let name = HSTRING::from(format!("Lumiere.NativeRegion.Rust.{thread}"));
        let class_desc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: PCWSTR(name.as_ptr()),
            // SAFETY: Shared system cursor is borrowed for the class lifetime.
            hCursor: unsafe { LoadCursorW(None, IDC_CROSS)? },
            ..Default::default()
        };
        // SAFETY: Static callback and owned class-name storage remain alive until unregister.
        if unsafe { RegisterClassExW(&class_desc) } == 0 {
            return Err(Error::from_thread());
        }
        let class = Class { name, instance };
        let mut state = Box::new(State {
            active: false,
            width: frame.width as f64,
            height: frame.height as f64,
            scale: 1.0,
            pointer: (0.0, 0.0),
            origin: None,
            selected: None,
            cancelled: false,
            dirty: true,
            capture: None,
        });
        let bounds = frame.monitor.bounds;
        // SAFETY: Stable boxed state is kept alive until the owned HWND is destroyed.
        let window = Window(unsafe {
            CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                PCWSTR(class.name.as_ptr()),
                w!("Lumiere Region"),
                WS_POPUP,
                bounds.left,
                bounds.top,
                frame.width as i32,
                frame.height as i32,
                None,
                None,
                Some(instance),
                Some((&mut *state as *mut State).cast()),
            )?
        });
        Ok(Self {
            presenter: None,
            size: (0, 0),
            window,
            state,
            _class: class,
        })
    }
    fn prepare(
        &mut self,
        device: &Device,
        shader: &VisualMatch,
        frame: &FrozenFrame,
    ) -> Result<()> {
        let bounds = frame.monitor.bounds;
        // SAFETY: Worker owns the hidden window and has per-monitor physical coordinate awareness.
        let (dpi, point) = unsafe {
            SetWindowPos(
                self.window.0,
                Some(HWND_TOPMOST),
                bounds.left,
                bounds.top,
                frame.width as i32,
                frame.height as i32,
                SWP_NOACTIVATE,
            )?;
            let dpi = GetDpiForWindow(self.window.0);
            let mut point = POINT::default();
            GetCursorPos(&mut point)?;
            (dpi, point)
        };
        if dpi == 0 {
            return Err(Error::new(E_FAIL, "Region window DPI is unavailable"));
        }
        *self.state = State {
            active: true,
            width: frame.width as f64,
            height: frame.height as f64,
            scale: dpi as f64 / 96.0,
            pointer: (
                (point.x - bounds.left) as f64,
                (point.y - bounds.top) as f64,
            ),
            origin: None,
            selected: None,
            cancelled: false,
            dirty: true,
            capture: None,
        };
        let surface = shader.render(
            device,
            &frame.texture,
            frame.width,
            frame.height,
            frame.color.scale,
        )?;
        if self.size != (frame.width, frame.height) {
            self.presenter = None;
        }
        if let Some(presenter) = &mut self.presenter {
            presenter.attach(&surface, self.state.scale as f32)?;
        } else {
            self.presenter = Some(presenter::Presenter::new(
                device,
                self.window.0,
                &surface,
                frame.width,
                frame.height,
                self.state.scale as f32,
            )?);
            self.size = (frame.width, frame.height);
        }
        self.presenter
            .as_ref()
            .expect("presenter initialized")
            .draw(None, self.state.pointer, false)?;
        // SAFETY: Present the matching frame while hidden, then activate exactly this worker-owned HWND.
        unsafe {
            SetWindowPos(
                self.window.0,
                Some(HWND_TOPMOST),
                bounds.left,
                bounds.top,
                frame.width as i32,
                frame.height as i32,
                SWP_SHOWWINDOW,
            )?;
            let foreground_thread = GetWindowThreadProcessId(GetForegroundWindow(), None);
            let current_thread = GetCurrentThreadId();
            let attached = foreground_thread != 0
                && foreground_thread != current_thread
                && AttachThreadInput(current_thread, foreground_thread, true).as_bool();
            let _ = SetForegroundWindow(self.window.0);
            let _ = SetFocus(Some(self.window.0));
            if attached {
                let _ = AttachThreadInput(current_thread, foreground_thread, false);
            }
        }
        Ok(())
    }
    pub(crate) fn finish(&mut self) {
        self.state.active = false;
        self.state.origin = None;
        self.state.capture = None;
        // SAFETY: Release only this worker's capture/visibility before detaching the frozen GPU surface.
        unsafe {
            if GetCapture() == self.window.0 {
                let _ = ReleaseCapture();
            }
            let _ = ShowWindow(self.window.0, SW_HIDE);
            let mut message = MSG::default();
            // Drop queued input from the completed selection before the window is reused.
            while PeekMessageW(&mut message, Some(self.window.0), 0, 0, PM_REMOVE).as_bool() {
                DispatchMessageW(&message);
            }
        }
        if let Some(presenter) = &mut self.presenter {
            presenter.detach();
        }
    }
    fn run(
        &mut self,
        frame: &FrozenFrame,
        cancel: &Cancellation,
        deadline: Instant,
    ) -> Result<Selection> {
        let mut checked_pointer = Instant::now();
        loop {
            if cancel.is_cancelled() || self.state.cancelled || Instant::now() >= deadline {
                return Ok(Selection::Cancelled);
            }
            if let Some(rect) = self.state.selected {
                self.presenter.as_ref().expect("prepared presenter").draw(
                    Some(rect),
                    self.state.pointer,
                    true,
                )?;
                self.state.active = false;
                return rect
                    .crop(
                        (self.state.width, self.state.height),
                        (frame.width, frame.height),
                    )
                    .map(Selection::Selected)
                    .ok_or_else(|| Error::new(E_FAIL, "Region selection has no source pixels"));
            }
            let mut message = MSG::default();
            // SAFETY: Dispatch only worker messages; no State borrow spans a native callback.
            unsafe {
                while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                    if message.message == WM_QUIT {
                        return Ok(Selection::Cancelled);
                    }
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                    if let Some(capture) = self.state.capture.take() {
                        if capture {
                            SetCapture(self.window.0);
                        } else {
                            let _ = ReleaseCapture();
                        }
                    }
                }
                if self.state.selected.is_some()
                    || checked_pointer.elapsed() >= Duration::from_millis(50)
                {
                    checked_pointer = Instant::now();
                    let mut point = POINT::default();
                    GetCursorPos(&mut point)?;
                    let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONULL);
                    if !monitor.0.is_null() && monitor != frame.monitor.handle {
                        return Ok(Selection::SwitchTarget);
                    }
                }
            }
            if self.state.dirty {
                self.state.dirty = false;
                self.presenter.as_ref().expect("prepared presenter").draw(
                    self.state.rect(),
                    self.state.pointer,
                    false,
                )?;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}
pub(crate) fn select(
    cache: &mut Option<Overlay>,
    device: &Device,
    shader: &VisualMatch,
    frame: &FrozenFrame,
    cancel: &Cancellation,
    deadline: Instant,
) -> Result<Selection> {
    if cancel.is_cancelled() || Instant::now() >= deadline {
        return Ok(Selection::Cancelled);
    }
    let bounds = frame.monitor.bounds;
    if bounds.right - bounds.left != frame.width as i32
        || bounds.bottom - bounds.top != frame.height as i32
    {
        return Err(Error::new(
            E_FAIL,
            "Region monitor geometry changed before selection",
        ));
    }
    if cache.is_none() {
        *cache = Some(Overlay::new(frame)?);
    }
    let overlay = cache.as_mut().expect("overlay initialized");
    let result = overlay
        .prepare(device, shader, frame)
        .and_then(|_| overlay.run(frame, cancel, deadline));
    // Keep the frozen selection/status visible through conversion and delivery.
    // The capture owner finishes it on every completion/error/cancellation path.
    if !matches!(&result, Ok(Selection::Selected(_))) {
        overlay.finish();
    }
    result
}
