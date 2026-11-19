//! Worker-owned native selection and annotation window. No frame readback precedes interaction.
pub(crate) mod annotations;
pub(crate) mod geometry;
mod icons;
mod presenter;
#[cfg(test)]
mod tests;
mod theme;
pub(crate) mod toolbar;
mod window_targets;

use crate::{Cancellation, capture::FrozenFrame, graphics::Device};
use annotations::{
    AnnotationHandle, AnnotationKind, AnnotationStore, Tool, constrain_arrow_angle,
    hit_test_selected_handle, merge_drag_rect,
};
use geometry::{Crop, Handle, Rect};
use std::time::{Duration, Instant};
use toolbar::{ToolbarHit, ToolbarLayout, ToolbarState};
use windows::{
    Win32::{
        Foundation::{E_FAIL, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
        Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromPoint, ValidateRect},
        System::{
            LibraryLoader::GetModuleHandleW,
            Threading::{AttachThreadInput, GetCurrentThreadId},
        },
        UI::{
            HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi},
            Input::KeyboardAndMouse::*,
            WindowsAndMessaging::*,
        },
    },
    core::{Error, HSTRING, PCWSTR, Result, w},
};

pub(crate) struct RegionCommit {
    pub crop: Crop,
    pub scale: f32,
    pub annotations: AnnotationStore,
}

pub(crate) enum Selection {
    Selected(RegionCommit),
    Cancelled,
    SwitchTarget,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Hover,
    DragSelection,
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CursorKind {
    Cross,
    Arrow,
    SizeAll,
    SizeNwSe,
    SizeNeSw,
    SizeNs,
    SizeWe,
}

#[derive(Clone, Debug)]
enum DragAction {
    NewSelection {
        origin: (f64, f64),
        moved: bool,
    },
    ResizeSelection {
        handle: Handle,
    },
    MoveSelection {
        last: (f64, f64),
    },
    ToolbarPress(ToolbarHit),
    DrawAnnotation {
        start: (f64, f64),
        kind: AnnotationKind,
    },
    MoveAnnotation {
        id: u64,
        last: (f64, f64),
        pushed_undo: bool,
    },
    ResizeAnnotation {
        id: u64,
        handle: AnnotationHandle,
        pushed_undo: bool,
    },
}

struct State {
    active: bool,
    width: f64,
    height: f64,
    scale: f64,
    pointer: (f64, f64),
    mode: Mode,
    drag_threshold: (f64, f64),
    windows: Vec<Rect>,
    locked: Option<Rect>,
    selection: Option<Rect>,
    committed: Option<Rect>,
    drag: Option<DragAction>,
    confirming_double_click: bool,
    annotations: AnnotationStore,
    toolbar: ToolbarState,
    cached_layout: Option<ToolbarLayout>,
    cancelled: bool,
    dirty: bool,
    capture: Option<bool>,
}

impl State {
    fn rect(&self) -> Option<Rect> {
        match self.mode {
            Mode::Hover => self.locked.or_else(|| self.hover()),
            Mode::DragSelection => match &self.drag {
                Some(DragAction::NewSelection { origin, .. }) => {
                    Rect::drag(*origin, self.pointer, self.width, self.height)
                }
                _ => self.selection,
            },
            Mode::Ready => self.committed.or(self.selection),
        }
    }

    fn allows_monitor_switch(&self) -> bool {
        self.mode == Mode::Hover && self.locked.is_none() && self.drag.is_none()
    }

    fn sync_layout(&mut self) {
        self.cached_layout = if self.mode == Mode::Ready {
            self.selection.map(|sel| {
                ToolbarLayout::compute(
                    sel,
                    (self.width, self.height),
                    self.scale,
                    &self.toolbar,
                    &self.annotations,
                )
            })
        } else {
            None
        };
    }

    fn hover(&self) -> Option<Rect> {
        // The first containing top-level window owns the point, even if too small to capture.
        self.windows
            .iter()
            .copied()
            .find(|r| r.contains(self.pointer))
            .filter(|r| r.is_valid_selection(self.scale))
    }

    fn handle_radius(&self) -> f64 {
        7.0 * self.scale
    }

    fn clear_drag(&mut self) {
        if self.drag.take().is_some() || self.confirming_double_click {
            self.confirming_double_click = false;
            self.toolbar.pressed = None;
            self.capture = Some(false);
        }
    }

    fn commit(&mut self) {
        if let Some(sel) = self.selection.filter(|r| r.is_valid_selection(self.scale)) {
            self.committed = Some(sel);
            self.annotations.selected_id = None;
            self.clear_drag();
            self.cached_layout = None;
            self.dirty = true;
        }
    }

    fn press(&mut self, shift: bool) {
        match self.mode {
            Mode::Hover => {
                if self.locked.is_none_or(|r| !r.contains(self.pointer)) {
                    self.locked = self.hover();
                }
                self.drag = Some(DragAction::NewSelection {
                    origin: self.pointer,
                    moved: false,
                });
                self.capture = Some(true);
                self.dirty = true;
            }
            Mode::DragSelection => {}
            Mode::Ready => {
                if let Some(hit) = self
                    .cached_layout
                    .as_ref()
                    .and_then(|l| l.hit_test(self.pointer))
                {
                    self.toolbar.pressed = Some(hit);
                    self.drag = Some(DragAction::ToolbarPress(hit));
                    self.capture = Some(true);
                    self.dirty = true;
                    return;
                }

                let Some(sel) = self.selection else {
                    return;
                };

                // Selected annotation handles take priority inside/around the selected object.
                if self.toolbar.tool == Tool::Select
                    && let Some((id, handle)) =
                        hit_test_selected_handle(&self.annotations, self.pointer, self.scale)
                {
                    self.drag = Some(DragAction::ResizeAnnotation {
                        id,
                        handle,
                        pushed_undo: false,
                    });
                    self.capture = Some(true);
                    self.dirty = true;
                    return;
                }

                // Selection crop handles remain draggable regardless of active drawing tool.
                if let Some(handle) = sel.hit_handle(self.pointer, self.handle_radius()) {
                    self.drag = Some(DragAction::ResizeSelection { handle });
                    self.capture = Some(true);
                    self.dirty = true;
                    return;
                }

                match self.toolbar.tool {
                    Tool::Select => {
                        if let Some(id) = self.annotations.hit_test(self.pointer, self.scale) {
                            self.annotations.selected_id = Some(id);
                            self.toolbar.subbar_open = true;
                            self.drag = Some(DragAction::MoveAnnotation {
                                id,
                                last: self.pointer,
                                pushed_undo: false,
                            });
                        } else {
                            self.annotations.selected_id = None;
                            self.toolbar.subbar_open = false;
                            if sel.contains_inclusive(self.pointer) {
                                self.drag = Some(DragAction::MoveSelection { last: self.pointer });
                            }
                        }
                    }
                    Tool::Rect | Tool::Spotlight | Tool::Arrow | Tool::Freehand => {
                        if !sel.contains_inclusive(self.pointer) {
                            return;
                        }
                        self.annotations.selected_id = None;
                        let start = (
                            self.pointer.0.clamp(sel.left, sel.right),
                            self.pointer.1.clamp(sel.top, sel.bottom),
                        );
                        let color = self.toolbar.color;
                        let width = self.toolbar.width;
                        let kind = match self.toolbar.tool {
                            Tool::Rect => AnnotationKind::Rect {
                                rect: Rect {
                                    left: start.0,
                                    top: start.1,
                                    right: start.0,
                                    bottom: start.1,
                                },
                                color,
                                width,
                            },
                            Tool::Spotlight => AnnotationKind::Spotlight {
                                rect: Rect {
                                    left: start.0,
                                    top: start.1,
                                    right: start.0,
                                    bottom: start.1,
                                },
                            },
                            Tool::Arrow => AnnotationKind::Arrow {
                                start,
                                end: if shift {
                                    constrain_arrow_angle(start, start)
                                } else {
                                    start
                                },
                                color,
                                width,
                            },
                            Tool::Freehand => AnnotationKind::Freehand {
                                points: vec![start],
                                color,
                                width,
                            },
                            Tool::Select => unreachable!(),
                        };
                        self.drag = Some(DragAction::DrawAnnotation { start, kind });
                    }
                }
                if self.drag.is_some() {
                    self.capture = Some(true);
                }
                self.sync_layout();
                self.dirty = true;
            }
        }
    }

    fn double_click(&mut self, shift: bool) {
        if self.mode == Mode::Ready {
            let on_toolbar = self
                .cached_layout
                .as_ref()
                .is_some_and(|l| l.hit_test(self.pointer).is_some());
            let on_handle = self
                .selection
                .is_some_and(|s| s.hit_handle(self.pointer, self.handle_radius()).is_some());
            if !on_toolbar
                && !on_handle
                && self.toolbar.tool == Tool::Select
                && self
                    .selection
                    .is_some_and(|s| s.contains_inclusive(self.pointer))
                && self
                    .annotations
                    .hit_test(self.pointer, self.scale)
                    .is_none()
            {
                // Commit on the final mouse-up, so it cannot reach the underlying application.
                self.confirming_double_click = true;
                self.capture = Some(true);
                return;
            }
        }
        self.press(shift);
    }

    fn pointer_move(&mut self, value: LPARAM, shift: bool) {
        let x = (value.0 as u16 as i16) as f64;
        let y = ((value.0 >> 16) as u16 as i16) as f64;
        self.pointer = (x.clamp(0.0, self.width), y.clamp(0.0, self.height));

        match self.drag.take() {
            Some(DragAction::NewSelection { origin, mut moved }) => {
                if !moved
                    && ((self.pointer.0 - origin.0).abs() >= self.drag_threshold.0
                        || (self.pointer.1 - origin.1).abs() >= self.drag_threshold.1)
                {
                    moved = true;
                    self.mode = Mode::DragSelection;
                    self.locked = None;
                }
                self.drag = Some(DragAction::NewSelection { origin, moved });
            }
            Some(DragAction::ResizeSelection { handle }) => {
                if let Some(sel) = self.selection {
                    self.selection = Some(sel.resize_with_handle(
                        handle,
                        self.pointer,
                        (self.width, self.height),
                        self.scale,
                        shift,
                    ));
                    self.sync_layout();
                }
                self.drag = Some(DragAction::ResizeSelection { handle });
            }
            Some(DragAction::MoveSelection { last }) => {
                if let Some(sel) = self.selection {
                    let delta = (self.pointer.0 - last.0, self.pointer.1 - last.1);
                    self.selection = Some(sel.translate(delta, (self.width, self.height)));
                    self.sync_layout();
                }
                self.drag = Some(DragAction::MoveSelection { last: self.pointer });
            }
            Some(DragAction::ToolbarPress(pressed)) => {
                self.toolbar.hovered = self
                    .cached_layout
                    .as_ref()
                    .and_then(|l| l.hit_test(self.pointer));
                self.drag = Some(DragAction::ToolbarPress(pressed));
            }
            Some(DragAction::DrawAnnotation { start, mut kind }) => {
                if let Some(sel) = self.selection {
                    let clamped = (
                        self.pointer.0.clamp(sel.left, sel.right),
                        self.pointer.1.clamp(sel.top, sel.bottom),
                    );
                    match &mut kind {
                        AnnotationKind::Rect { rect, .. } | AnnotationKind::Spotlight { rect } => {
                            if let Some(next) = merge_drag_rect(start, clamped, sel, shift) {
                                *rect = next;
                            }
                        }
                        AnnotationKind::Arrow { end, .. } => {
                            let target = if shift {
                                constrain_arrow_angle(start, clamped)
                            } else {
                                clamped
                            };
                            *end = (
                                target.0.clamp(sel.left, sel.right),
                                target.1.clamp(sel.top, sel.bottom),
                            );
                        }
                        AnnotationKind::Freehand { points, .. } => {
                            if let Some(prev) = points.last() {
                                let dx = clamped.0 - prev.0;
                                let dy = clamped.1 - prev.1;
                                if dx * dx + dy * dy >= (2.0 * self.scale).powi(2) {
                                    points.push(clamped);
                                }
                            }
                        }
                    }
                }
                self.drag = Some(DragAction::DrawAnnotation { start, kind });
            }
            Some(DragAction::MoveAnnotation {
                id,
                last,
                mut pushed_undo,
            }) => {
                if let Some(sel) = self.selection {
                    let delta = (self.pointer.0 - last.0, self.pointer.1 - last.1);
                    if !pushed_undo {
                        let before = self.annotations.items.clone();
                        if let Some(item) = self.annotations.items.iter_mut().find(|i| i.id == id)
                            && item.kind.translate_clamped(delta, sel)
                        {
                            let current = std::mem::replace(&mut self.annotations.items, before);
                            self.annotations.push_undo_checkpoint();
                            self.annotations.items = current;
                            pushed_undo = true;
                        }
                    } else if let Some(item) =
                        self.annotations.items.iter_mut().find(|i| i.id == id)
                    {
                        item.kind.translate_clamped(delta, sel);
                    }
                }
                self.drag = Some(DragAction::MoveAnnotation {
                    id,
                    last: self.pointer,
                    pushed_undo,
                });
            }
            Some(DragAction::ResizeAnnotation {
                id,
                handle,
                mut pushed_undo,
            }) => {
                if let Some(sel) = self.selection {
                    if !pushed_undo {
                        let before = self.annotations.items.clone();
                        if let Some(item) = self.annotations.items.iter_mut().find(|i| i.id == id)
                            && item
                                .kind
                                .resize_with_handle(handle, self.pointer, sel, self.scale)
                        {
                            let current = std::mem::replace(&mut self.annotations.items, before);
                            self.annotations.push_undo_checkpoint();
                            self.annotations.items = current;
                            pushed_undo = true;
                        }
                    } else if let Some(item) =
                        self.annotations.items.iter_mut().find(|i| i.id == id)
                    {
                        item.kind
                            .resize_with_handle(handle, self.pointer, sel, self.scale);
                    }
                }
                self.drag = Some(DragAction::ResizeAnnotation {
                    id,
                    handle,
                    pushed_undo,
                });
            }
            None => {
                if self.mode == Mode::Ready {
                    self.toolbar.hovered = self
                        .cached_layout
                        .as_ref()
                        .and_then(|l| l.hit_test(self.pointer));
                }
            }
        }
        self.dirty = true;
    }

    fn release(&mut self) {
        self.capture = Some(false);
        if self.confirming_double_click {
            self.confirming_double_click = false;
            if self
                .selection
                .is_some_and(|s| s.contains_inclusive(self.pointer))
            {
                self.commit();
            }
            return;
        }

        match self.drag.take() {
            Some(DragAction::NewSelection { origin, moved }) => {
                if moved {
                    let dragged = Rect::drag(origin, self.pointer, self.width, self.height)
                        .filter(|r| r.is_valid_selection(self.scale));
                    if let Some(rect) = dragged {
                        self.selection = Some(rect);
                        self.mode = Mode::Ready;
                        self.locked = None;
                    } else {
                        self.selection = None;
                        self.mode = Mode::Hover;
                        self.locked = None;
                    }
                } else if let Some(locked) = self.locked.take() {
                    self.selection = Some(locked);
                    self.mode = Mode::Ready;
                } else {
                    self.mode = Mode::Hover;
                }
            }
            Some(DragAction::ToolbarPress(pressed)) => {
                self.toolbar.pressed = None;
                let current = self
                    .cached_layout
                    .as_ref()
                    .and_then(|l| l.hit_test(self.pointer));
                self.toolbar.hovered = current;
                if current == Some(pressed) {
                    self.activate_toolbar_hit(pressed);
                }
            }
            Some(DragAction::DrawAnnotation { kind, .. }) => {
                if kind.is_meaningful(self.scale) {
                    self.annotations.add(kind);
                    self.annotations.selected_id = None;
                }
            }
            Some(
                DragAction::ResizeSelection { .. }
                | DragAction::MoveSelection { .. }
                | DragAction::MoveAnnotation { .. }
                | DragAction::ResizeAnnotation { .. },
            )
            | None => {}
        }
        self.sync_layout();
        self.dirty = true;
    }

    fn activate_toolbar_hit(&mut self, hit: ToolbarHit) {
        match hit {
            ToolbarHit::Tool(tool) => {
                self.toolbar.set_tool(tool);
                if tool != Tool::Select {
                    self.annotations.selected_id = None;
                }
            }
            ToolbarHit::AttributeToggle => {
                self.toolbar.subbar_open = !self.toolbar.subbar_open;
            }
            ToolbarHit::Undo => {
                self.annotations.undo();
            }
            ToolbarHit::Redo => {
                self.annotations.redo();
            }
            ToolbarHit::Cancel => {
                self.cancelled = true;
            }
            ToolbarHit::Confirm => {
                self.commit();
            }
            ToolbarHit::ColorSwatch(color) => {
                self.toolbar.color = color;
                if self.toolbar.tool == Tool::Select
                    && self
                        .annotations
                        .selected()
                        .is_some_and(|a| !a.kind.is_spotlight())
                {
                    let before = self.annotations.items.clone();
                    if self
                        .annotations
                        .selected_mut()
                        .is_some_and(|s| s.kind.set_color(color))
                    {
                        let current = std::mem::replace(&mut self.annotations.items, before);
                        self.annotations.push_undo_checkpoint();
                        self.annotations.items = current;
                    }
                }
            }
            ToolbarHit::StrokeWidth(width) => {
                self.toolbar.width = width;
                if self.toolbar.tool == Tool::Select
                    && self
                        .annotations
                        .selected()
                        .is_some_and(|a| !a.kind.is_spotlight())
                {
                    let before = self.annotations.items.clone();
                    if self
                        .annotations
                        .selected_mut()
                        .is_some_and(|s| s.kind.set_width(width))
                    {
                        let current = std::mem::replace(&mut self.annotations.items, before);
                        self.annotations.push_undo_checkpoint();
                        self.annotations.items = current;
                    }
                }
            }
            ToolbarHit::SpotlightDim(dim) => {
                if self.annotations.spotlight_dim != dim {
                    if self.annotations.has_spotlights() {
                        self.annotations.push_undo_checkpoint();
                    }
                    self.annotations.spotlight_dim = dim;
                }
            }
            ToolbarHit::ClearSpotlights => {
                self.annotations.clear_spotlights();
            }
            ToolbarHit::BarBody => {}
        }
        self.sync_layout();
        self.dirty = true;
    }

    fn right_click(&mut self) {
        match self.mode {
            Mode::Hover | Mode::DragSelection => {
                self.clear_drag();
                self.cancelled = true;
            }
            Mode::Ready => {
                if self.drag.is_some() {
                    self.clear_drag();
                } else if self.toolbar.tool != Tool::Select {
                    self.toolbar.set_tool(Tool::Select);
                } else if self.annotations.selected_id.is_some() {
                    self.annotations.selected_id = None;
                    self.toolbar.subbar_open = false;
                } else {
                    self.cancelled = true;
                }
                self.sync_layout();
            }
        }
        self.dirty = true;
    }

    fn key_down(&mut self, vk: u16, ctrl: bool, shift: bool, alt: bool) {
        if vk == VK_ESCAPE.0 {
            if self.drag.is_some() {
                self.clear_drag();
            } else if self.annotations.selected_id.is_some() {
                self.annotations.selected_id = None;
                self.toolbar.subbar_open = false;
                self.sync_layout();
            } else {
                self.cancelled = true;
            }
            self.dirty = true;
            return;
        }
        if self.mode != Mode::Ready {
            return;
        }
        if vk == VK_RETURN.0 {
            self.commit();
            return;
        }
        if vk == VK_DELETE.0 || vk == VK_BACK.0 {
            if self.annotations.delete_selected() {
                self.sync_layout();
                self.dirty = true;
            }
            return;
        }
        if ctrl && !alt {
            match vk {
                0x5A => {
                    // Ctrl+Z / Ctrl+Shift+Z
                    if shift {
                        self.annotations.redo();
                    } else {
                        self.annotations.undo();
                    }
                    self.sync_layout();
                    self.dirty = true;
                }
                0x59 => {
                    // Ctrl+Y
                    self.annotations.redo();
                    self.sync_layout();
                    self.dirty = true;
                }
                0x43 => {
                    // Ctrl+C
                    self.commit();
                }
                _ => {}
            }
            return;
        }
        if !ctrl && !alt {
            let next_tool = match vk {
                0x56 => Some(Tool::Select),    // V
                0x52 => Some(Tool::Rect),      // R
                0x48 => Some(Tool::Spotlight), // H
                0x41 => Some(Tool::Arrow),     // A
                0x50 => Some(Tool::Freehand),  // P
                _ => None,
            };
            if let Some(tool) = next_tool {
                self.clear_drag();
                self.toolbar.tool = tool;
                self.toolbar.subbar_open = tool != Tool::Select;
                if tool != Tool::Select {
                    self.annotations.selected_id = None;
                }
                self.sync_layout();
                self.dirty = true;
            }
        }
    }

    fn phase(&self) -> presenter::Phase {
        match self.mode {
            Mode::Hover => presenter::Phase::Hover,
            Mode::DragSelection => presenter::Phase::Drag,
            Mode::Ready => {
                if self.committed.is_some() {
                    presenter::Phase::Capture
                } else {
                    presenter::Phase::Ready
                }
            }
        }
    }

    fn draft(&self) -> Option<&AnnotationKind> {
        match &self.drag {
            Some(DragAction::DrawAnnotation { kind, .. }) => Some(kind),
            _ => None,
        }
    }

    fn cursor_kind(&self) -> CursorKind {
        if self.mode != Mode::Ready {
            return CursorKind::Cross;
        }
        if self
            .cached_layout
            .as_ref()
            .is_some_and(|l| l.hit_test(self.pointer).is_some())
        {
            return CursorKind::Arrow;
        }
        if let Some(DragAction::ResizeSelection { handle }) = &self.drag {
            return handle_cursor(*handle);
        }
        if let Some(DragAction::MoveSelection { .. }) = &self.drag {
            return CursorKind::SizeAll;
        }
        if self.toolbar.tool == Tool::Select
            && let Some((_, handle)) =
                hit_test_selected_handle(&self.annotations, self.pointer, self.scale)
        {
            return match handle {
                AnnotationHandle::Corner(h) => handle_cursor(h),
                AnnotationHandle::ArrowStart | AnnotationHandle::ArrowEnd => CursorKind::SizeAll,
            };
        }
        if let Some(sel) = self.selection
            && let Some(handle) = sel.hit_handle(self.pointer, self.handle_radius())
        {
            return handle_cursor(handle);
        }
        if self.toolbar.tool == Tool::Select {
            if self
                .annotations
                .hit_test(self.pointer, self.scale)
                .is_some()
                || self
                    .selection
                    .is_some_and(|s| s.contains_inclusive(self.pointer))
            {
                return CursorKind::SizeAll;
            }
            return CursorKind::Arrow;
        }
        CursorKind::Cross
    }
}

fn handle_cursor(handle: Handle) -> CursorKind {
    match handle {
        Handle::TopLeft | Handle::BottomRight => CursorKind::SizeNwSe,
        Handle::TopRight | Handle::BottomLeft => CursorKind::SizeNeSw,
        Handle::Top | Handle::Bottom => CursorKind::SizeNs,
        Handle::Left | Handle::Right => CursorKind::SizeWe,
    }
}

fn key_down_now(vk: VIRTUAL_KEY) -> bool {
    // SAFETY: Querying instantaneous key state on the window thread.
    unsafe { (GetKeyState(i32::from(vk.0)) as u16 & 0x8000) != 0 }
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
            if (!state.active || state.committed.is_some())
                && matches!(
                    message,
                    WM_LBUTTONDOWN
                        | WM_LBUTTONDBLCLK
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
                WM_SETCURSOR if (lparam.0 & 0xFFFF) as u32 == HTCLIENT => {
                    let id = match state.cursor_kind() {
                        CursorKind::Cross => IDC_CROSS,
                        CursorKind::Arrow => IDC_ARROW,
                        CursorKind::SizeAll => IDC_SIZEALL,
                        CursorKind::SizeNwSe => IDC_SIZENWSE,
                        CursorKind::SizeNeSw => IDC_SIZENESW,
                        CursorKind::SizeNs => IDC_SIZENS,
                        CursorKind::SizeWe => IDC_SIZEWE,
                    };
                    if let Ok(cursor) = LoadCursorW(None, id) {
                        SetCursor(Some(cursor));
                    }
                    return LRESULT(1);
                }
                WM_CLOSE | WM_DESTROY | WM_DISPLAYCHANGE | WM_DPICHANGED => {
                    state.cancelled = true;
                    return LRESULT(0);
                }
                WM_RBUTTONDOWN => {
                    state.pointer_move(lparam, key_down_now(VK_SHIFT));
                    state.right_click();
                    return LRESULT(0);
                }
                WM_KEYDOWN => {
                    state.key_down(
                        wparam.0 as u16,
                        key_down_now(VK_CONTROL),
                        key_down_now(VK_SHIFT),
                        key_down_now(VK_MENU),
                    );
                    return LRESULT(0);
                }
                WM_LBUTTONDOWN => {
                    let shift = key_down_now(VK_SHIFT);
                    state.pointer_move(lparam, shift);
                    state.press(shift);
                    return LRESULT(0);
                }
                WM_LBUTTONDBLCLK => {
                    let shift = key_down_now(VK_SHIFT);
                    state.pointer_move(lparam, shift);
                    state.double_click(shift);
                    return LRESULT(0);
                }
                WM_MOUSEMOVE => {
                    state.pointer_move(lparam, key_down_now(VK_SHIFT));
                    return LRESULT(0);
                }
                WM_LBUTTONUP if state.drag.is_some() || state.confirming_double_click => {
                    state.pointer_move(lparam, key_down_now(VK_SHIFT));
                    state.release();
                    return LRESULT(0);
                }
                WM_CAPTURECHANGED if state.drag.is_some() => {
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
            style: CS_DBLCLKS,
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
            width: f64::from(frame.width),
            height: f64::from(frame.height),
            scale: 1.0,
            pointer: (0.0, 0.0),
            mode: Mode::Hover,
            drag_threshold: (4.0, 4.0),
            windows: Vec::new(),
            locked: None,
            selection: None,
            committed: None,
            drag: None,
            confirming_double_click: false,
            annotations: AnnotationStore::default(),
            toolbar: ToolbarState::default(),
            cached_layout: None,
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

    fn prepare(&mut self, device: &Device, frame: &FrozenFrame) -> Result<()> {
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
            width: f64::from(frame.width),
            height: f64::from(frame.height),
            scale: f64::from(dpi) / 96.0,
            pointer: (
                f64::from(point.x - bounds.left),
                f64::from(point.y - bounds.top),
            ),
            mode: Mode::Hover,
            // SAFETY: System metrics are read on the worker's per-monitor-aware thread.
            drag_threshold: unsafe {
                (
                    f64::from(GetSystemMetricsForDpi(SM_CXDRAG, dpi).max(1)),
                    f64::from(GetSystemMetricsForDpi(SM_CYDRAG, dpi).max(1)),
                )
            },
            windows: window_targets::snapshot(self.window.0, bounds).unwrap_or_else(|error| {
                crate::native_failure("region-window-snapshot", error);
                Vec::new()
            }),
            locked: None,
            selection: None,
            committed: None,
            drag: None,
            confirming_double_click: false,
            annotations: AnnotationStore::default(),
            toolbar: ToolbarState::default(),
            cached_layout: None,
            cancelled: false,
            dirty: true,
            capture: None,
        };
        // Captured scRGB already contains the target's SDR white scaling. Apply it
        // only to UI colors, never to the immutable captured background.
        let white_scale = frame.color.scale.recip();
        if self.size != (frame.width, frame.height) {
            self.presenter = None;
        }
        if let Some(presenter) = &mut self.presenter {
            presenter.attach(&frame.texture, self.state.scale as f32, white_scale)?;
        } else {
            self.presenter = Some(presenter::Presenter::new(
                device,
                self.window.0,
                &frame.texture,
                frame.width,
                frame.height,
                self.state.scale as f32,
                white_scale,
            )?);
            self.size = (frame.width, frame.height);
        }
        self.draw_current()?;
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

    fn draw_current(&self) -> Result<()> {
        let presenter = self.presenter.as_ref().expect("prepared presenter");
        let toolbar_pair = self
            .state
            .cached_layout
            .as_ref()
            .map(|l| (&self.state.toolbar, l));
        presenter.draw(
            self.state.rect(),
            self.state.pointer,
            self.state.phase(),
            &self.state.annotations,
            self.state.draft(),
            toolbar_pair,
        )
    }

    pub(crate) fn finish(&mut self) {
        self.state.active = false;
        self.state.mode = Mode::Hover;
        self.state.locked = None;
        self.state.selection = None;
        self.state.committed = None;
        self.state.drag = None;
        self.state.confirming_double_click = false;
        self.state.windows.clear();
        self.state.annotations.reset();
        self.state.toolbar.reset();
        self.state.cached_layout = None;
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
            if let Some(rect) = self.state.committed {
                self.presenter.as_ref().expect("prepared presenter").draw(
                    Some(rect),
                    self.state.pointer,
                    presenter::Phase::Capture,
                    &self.state.annotations,
                    None,
                    None,
                )?;
                self.state.active = false;
                let crop = rect
                    .crop(
                        (self.state.width, self.state.height),
                        (frame.width, frame.height),
                    )
                    .ok_or_else(|| Error::new(E_FAIL, "Region selection has no source pixels"))?;
                let annotations = std::mem::take(&mut self.state.annotations);
                return Ok(Selection::Selected(RegionCommit {
                    crop,
                    scale: self.state.scale as f32,
                    annotations,
                }));
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
                if self.state.allows_monitor_switch()
                    && checked_pointer.elapsed() >= Duration::from_millis(50)
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
            if self.state.committed.is_some() {
                continue;
            }
            if self.state.dirty {
                self.state.dirty = false;
                self.draw_current()?;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

pub(crate) fn select(
    cache: &mut Option<Overlay>,
    device: &Device,
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
        .prepare(device, frame)
        .and_then(|_| overlay.run(frame, cancel, deadline));
    // Keep the frozen selection/status visible through conversion and delivery.
    // The capture owner finishes it on every completion/error/cancellation path.
    if !matches!(&result, Ok(Selection::Selected(_))) {
        overlay.finish();
    }
    result
}
