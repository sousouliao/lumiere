use super::{
    geometry::{Crop, Handle, Rect},
    theme::{RenderResources, srgb_to_linear},
};
use crate::graphics::Device;
use windows::{
    Win32::{
        Foundation::E_FAIL,
        Graphics::{
            Direct2D::{Common::*, *},
            Direct3D11::*,
            Dxgi::{Common::*, IDXGIDevice, IDXGISurface},
        },
    },
    core::{Error, Interface, Result},
};
use windows_numerics::Vector2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tool {
    Select,
    Rect,
    Spotlight,
    Arrow,
    Freehand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InkColor {
    Coral,
    Amber,
    Mint,
    Cerulean,
    Chalk,
    Charcoal,
}

impl InkColor {
    pub const ALL: [Self; 6] = [
        Self::Coral,
        Self::Amber,
        Self::Mint,
        Self::Cerulean,
        Self::Chalk,
        Self::Charcoal,
    ];

    pub fn srgb(self) -> (f32, f32, f32) {
        match self {
            Self::Coral => (232.0 / 255.0, 102.0 / 255.0, 90.0 / 255.0),
            Self::Amber => (240.0 / 255.0, 180.0 / 255.0, 76.0 / 255.0),
            Self::Mint => (107.0 / 255.0, 196.0 / 255.0, 132.0 / 255.0),
            Self::Cerulean => (78.0 / 255.0, 161.0 / 255.0, 243.0 / 255.0),
            Self::Chalk => (247.0 / 255.0, 245.0 / 255.0, 240.0 / 255.0),
            Self::Charcoal => (24.0 / 255.0, 22.0 / 255.0, 21.0 / 255.0),
        }
    }

    pub fn d2d_color(self, white_scale: f32, is_export: bool) -> D2D1_COLOR_F {
        let (r, g, b) = self.srgb();
        if is_export {
            D2D1_COLOR_F { r, g, b, a: 1.0 }
        } else {
            D2D1_COLOR_F {
                r: srgb_to_linear(r) * white_scale,
                g: srgb_to_linear(g) * white_scale,
                b: srgb_to_linear(b) * white_scale,
                a: 1.0,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StrokeWidth {
    Small,
    Medium,
    Large,
}

impl StrokeWidth {
    pub const ALL: [Self; 3] = [Self::Small, Self::Medium, Self::Large];

    pub fn dip(self) -> f32 {
        match self {
            Self::Small => 2.0,
            Self::Medium => 3.5,
            Self::Large => 6.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpotlightDim {
    Soft,
    Standard,
    Deep,
}

impl SpotlightDim {
    pub const ALL: [Self; 3] = [Self::Soft, Self::Standard, Self::Deep];

    pub fn srgb_alpha(self) -> f32 {
        match self {
            Self::Soft => 0.40,
            Self::Standard => 0.55,
            Self::Deep => 0.72,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Soft => "40%",
            Self::Standard => "55%",
            Self::Deep => "72%",
        }
    }

    pub fn veil_alpha(self, is_export: bool) -> f32 {
        let alpha = self.srgb_alpha();
        if is_export {
            alpha
        } else {
            // Match physical light attenuation between linear FP16 preview and sRGB PNG output.
            1.0 - srgb_to_linear(1.0 - alpha)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AnnotationKind {
    Rect {
        rect: Rect,
        color: InkColor,
        width: StrokeWidth,
    },
    Spotlight {
        rect: Rect,
    },
    Arrow {
        start: (f64, f64),
        end: (f64, f64),
        color: InkColor,
        width: StrokeWidth,
    },
    Freehand {
        points: Vec<(f64, f64)>,
        color: InkColor,
        width: StrokeWidth,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Annotation {
    pub id: u64,
    pub kind: AnnotationKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AnnotationHandle {
    Corner(Handle),
    ArrowStart,
    ArrowEnd,
}

impl AnnotationKind {
    pub fn is_spotlight(&self) -> bool {
        matches!(self, Self::Spotlight { .. })
    }

    pub fn is_meaningful(&self, scale: f64) -> bool {
        let min_span = 4.0 * scale;
        match self {
            Self::Rect { rect, .. } | Self::Spotlight { rect } => {
                rect.width() >= min_span && rect.height() >= min_span
            }
            Self::Arrow { start, end, .. } => {
                let dx = end.0 - start.0;
                let dy = end.1 - start.1;
                (dx * dx + dy * dy).sqrt() >= 8.0 * scale
            }
            Self::Freehand { points, .. } => points.len() >= 2,
        }
    }

    pub fn translate_clamped(&mut self, delta: (f64, f64), selection: Rect) -> bool {
        let bounds = self.bounds();
        let min_dx = (selection.left - bounds.left).min(0.0);
        let max_dx = (selection.right - bounds.right).max(0.0);
        let min_dy = (selection.top - bounds.top).min(0.0);
        let max_dy = (selection.bottom - bounds.bottom).max(0.0);
        let dx = delta.0.clamp(min_dx, max_dx);
        let dy = delta.1.clamp(min_dy, max_dy);
        if dx.abs() < 1e-6 && dy.abs() < 1e-6 {
            return false;
        }
        match self {
            Self::Rect { rect, .. } | Self::Spotlight { rect } => {
                rect.left += dx;
                rect.top += dy;
                rect.right += dx;
                rect.bottom += dy;
            }
            Self::Arrow { start, end, .. } => {
                start.0 += dx;
                start.1 += dy;
                end.0 += dx;
                end.1 += dy;
            }
            Self::Freehand { points, .. } => {
                for p in points {
                    p.0 += dx;
                    p.1 += dy;
                }
            }
        }
        true
    }

    pub fn bounds(&self) -> Rect {
        match self {
            Self::Rect { rect, .. } | Self::Spotlight { rect } => *rect,
            Self::Arrow { start, end, .. } => Rect {
                left: start.0.min(end.0),
                top: start.1.min(end.1),
                right: start.0.max(end.0),
                bottom: start.1.max(end.1),
            },
            Self::Freehand { points, .. } => {
                let mut left = f64::INFINITY;
                let mut top = f64::INFINITY;
                let mut right = f64::NEG_INFINITY;
                let mut bottom = f64::NEG_INFINITY;
                for &(x, y) in points {
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x);
                    bottom = bottom.max(y);
                }
                if !left.is_finite() {
                    Rect {
                        left: 0.0,
                        top: 0.0,
                        right: 0.0,
                        bottom: 0.0,
                    }
                } else {
                    Rect {
                        left,
                        top,
                        right,
                        bottom,
                    }
                }
            }
        }
    }

    pub fn handles(&self) -> Vec<(AnnotationHandle, (f64, f64))> {
        match self {
            Self::Rect { rect, .. } | Self::Spotlight { rect } => vec![
                (
                    AnnotationHandle::Corner(Handle::TopLeft),
                    (rect.left, rect.top),
                ),
                (
                    AnnotationHandle::Corner(Handle::TopRight),
                    (rect.right, rect.top),
                ),
                (
                    AnnotationHandle::Corner(Handle::BottomRight),
                    (rect.right, rect.bottom),
                ),
                (
                    AnnotationHandle::Corner(Handle::BottomLeft),
                    (rect.left, rect.bottom),
                ),
            ],
            Self::Arrow { start, end, .. } => vec![
                (AnnotationHandle::ArrowStart, *start),
                (AnnotationHandle::ArrowEnd, *end),
            ],
            Self::Freehand { .. } => Vec::new(),
        }
    }

    pub fn hit_handle(&self, point: (f64, f64), radius: f64) -> Option<AnnotationHandle> {
        self.handles().into_iter().find_map(|(handle, pos)| {
            ((point.0 - pos.0).abs() <= radius && (point.1 - pos.1).abs() <= radius)
                .then_some(handle)
        })
    }

    pub fn resize_with_handle(
        &mut self,
        handle: AnnotationHandle,
        pointer: (f64, f64),
        selection: Rect,
        scale: f64,
    ) -> bool {
        let min_span = 4.0 * scale;
        let px = pointer.0.clamp(selection.left, selection.right);
        let py = pointer.1.clamp(selection.top, selection.bottom);
        let before = self.clone();
        match (&mut *self, handle) {
            (
                Self::Rect { rect, .. } | Self::Spotlight { rect },
                AnnotationHandle::Corner(corner),
            ) => match corner {
                Handle::TopLeft => {
                    rect.left = px.min(rect.right - min_span);
                    rect.top = py.min(rect.bottom - min_span);
                }
                Handle::TopRight => {
                    rect.right = px.max(rect.left + min_span);
                    rect.top = py.min(rect.bottom - min_span);
                }
                Handle::BottomRight => {
                    rect.right = px.max(rect.left + min_span);
                    rect.bottom = py.max(rect.top + min_span);
                }
                Handle::BottomLeft => {
                    rect.left = px.min(rect.right - min_span);
                    rect.bottom = py.max(rect.top + min_span);
                }
                _ => {}
            },
            (Self::Arrow { start, .. }, AnnotationHandle::ArrowStart) => {
                *start = (px, py);
            }
            (Self::Arrow { end, .. }, AnnotationHandle::ArrowEnd) => {
                *end = (px, py);
            }
            _ => return false,
        }
        before != *self
    }

    pub fn hit_test(&self, point: (f64, f64), scale: f64) -> bool {
        let pad = 6.0 * scale;
        match self {
            Self::Rect { rect, width, .. } => {
                let half = f64::from(width.dip()) * 0.5 * scale + pad;
                let outer = Rect {
                    left: rect.left - half,
                    top: rect.top - half,
                    right: rect.right + half,
                    bottom: rect.bottom + half,
                };
                let inner = Rect {
                    left: rect.left + half,
                    top: rect.top + half,
                    right: rect.right - half,
                    bottom: rect.bottom - half,
                };
                outer.contains_inclusive(point) && !inner.contains(point)
            }
            Self::Spotlight { rect } => rect.contains_inclusive(point),
            Self::Arrow {
                start, end, width, ..
            } => {
                let tol = f64::from(width.dip()) * 0.5 * scale + pad + 4.0 * scale;
                point_segment_distance(point, *start, *end) <= tol
            }
            Self::Freehand { points, width, .. } => {
                let tol = f64::from(width.dip()) * 0.5 * scale + pad;
                points
                    .windows(2)
                    .any(|seg| point_segment_distance(point, seg[0], seg[1]) <= tol)
            }
        }
    }

    pub fn set_color(&mut self, next: InkColor) -> bool {
        match self {
            Self::Rect { color, .. } | Self::Arrow { color, .. } | Self::Freehand { color, .. } => {
                let changed = *color != next;
                *color = next;
                changed
            }
            Self::Spotlight { .. } => false,
        }
    }

    pub fn set_width(&mut self, next: StrokeWidth) -> bool {
        match self {
            Self::Rect { width, .. } | Self::Arrow { width, .. } | Self::Freehand { width, .. } => {
                let changed = *width != next;
                *width = next;
                changed
            }
            Self::Spotlight { .. } => false,
        }
    }
}

#[derive(Clone, Debug, Default)]
struct Snapshot {
    items: Vec<Annotation>,
    spotlight_dim: Option<SpotlightDim>,
}

#[derive(Clone, Debug)]
pub(crate) struct AnnotationStore {
    pub items: Vec<Annotation>,
    pub spotlight_dim: SpotlightDim,
    pub selected_id: Option<u64>,
    next_id: u64,
    undo_stack: Vec<Snapshot>,
    redo_stack: Vec<Snapshot>,
}

impl Default for AnnotationStore {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            spotlight_dim: SpotlightDim::Standard,
            selected_id: None,
            next_id: 1,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }
}

impl AnnotationStore {
    pub fn reset(&mut self) {
        self.items.clear();
        self.spotlight_dim = SpotlightDim::Standard;
        self.selected_id = None;
        self.next_id = 1;
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn has_annotations(&self) -> bool {
        !self.items.is_empty()
    }

    pub fn has_spotlights(&self) -> bool {
        self.items.iter().any(|item| item.kind.is_spotlight())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn push_undo_checkpoint(&mut self) {
        self.undo_stack.push(Snapshot {
            items: self.items.clone(),
            spotlight_dim: Some(self.spotlight_dim),
        });
        if self.undo_stack.len() > 64 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn add(&mut self, kind: AnnotationKind) -> u64 {
        self.push_undo_checkpoint();
        let id = self.next_id;
        self.next_id += 1;
        self.items.push(Annotation { id, kind });
        self.selected_id = Some(id);
        id
    }

    pub fn undo(&mut self) -> bool {
        let Some(prev) = self.undo_stack.pop() else {
            return false;
        };
        self.redo_stack.push(Snapshot {
            items: self.items.clone(),
            spotlight_dim: Some(self.spotlight_dim),
        });
        self.items = prev.items;
        if let Some(dim) = prev.spotlight_dim {
            self.spotlight_dim = dim;
        }
        if !self
            .items
            .iter()
            .any(|item| Some(item.id) == self.selected_id)
        {
            self.selected_id = None;
        }
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo_stack.pop() else {
            return false;
        };
        self.undo_stack.push(Snapshot {
            items: self.items.clone(),
            spotlight_dim: Some(self.spotlight_dim),
        });
        self.items = next.items;
        if let Some(dim) = next.spotlight_dim {
            self.spotlight_dim = dim;
        }
        if !self
            .items
            .iter()
            .any(|item| Some(item.id) == self.selected_id)
        {
            self.selected_id = None;
        }
        true
    }

    pub fn selected(&self) -> Option<&Annotation> {
        let id = self.selected_id?;
        self.items.iter().find(|item| item.id == id)
    }

    pub fn selected_mut(&mut self) -> Option<&mut Annotation> {
        let id = self.selected_id?;
        self.items.iter_mut().find(|item| item.id == id)
    }

    pub fn delete_selected(&mut self) -> bool {
        let Some(id) = self.selected_id else {
            return false;
        };
        if let Some(pos) = self.items.iter().position(|item| item.id == id) {
            self.push_undo_checkpoint();
            self.items.remove(pos);
            self.selected_id = None;
            true
        } else {
            self.selected_id = None;
            false
        }
    }

    pub fn clear_spotlights(&mut self) -> bool {
        if !self.has_spotlights() {
            return false;
        }
        self.push_undo_checkpoint();
        self.items.retain(|item| !item.kind.is_spotlight());
        if self.selected().is_none() {
            self.selected_id = None;
        }
        true
    }

    /// Hit-test topmost vector strokes first, then spotlight cutouts underneath.
    pub fn hit_test(&self, point: (f64, f64), scale: f64) -> Option<u64> {
        if let Some(hit) = self
            .items
            .iter()
            .rev()
            .find(|item| !item.kind.is_spotlight() && item.kind.hit_test(point, scale))
        {
            return Some(hit.id);
        }
        self.items
            .iter()
            .rev()
            .find(|item| item.kind.is_spotlight() && item.kind.hit_test(point, scale))
            .map(|item| item.id)
    }
}

pub(crate) fn constrain_arrow_angle(start: (f64, f64), end: (f64, f64)) -> (f64, f64) {
    let dx = end.0 - start.0;
    let dy = end.1 - start.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len <= 0.001 {
        return end;
    }
    let angle = dy.atan2(dx);
    let step = std::f64::consts::FRAC_PI_4;
    let snapped = (angle / step).round() * step;
    (start.0 + len * snapped.cos(), start.1 + len * snapped.sin())
}

fn point_segment_distance(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let vx = b.0 - a.0;
    let vy = b.1 - a.1;
    let len_sq = vx * vx + vy * vy;
    if len_sq <= 1e-6 {
        let dx = p.0 - a.0;
        let dy = p.1 - a.1;
        return (dx * dx + dy * dy).sqrt();
    }
    let t = (((p.0 - a.0) * vx + (p.1 - a.1) * vy) / len_sq).clamp(0.0, 1.0);
    let proj_x = a.0 + t * vx;
    let proj_y = a.1 + t * vy;
    let dx = p.0 - proj_x;
    let dy = p.1 - proj_y;
    (dx * dx + dy * dy).sqrt()
}

fn d2d_rect(r: Rect, origin: (f64, f64)) -> D2D_RECT_F {
    D2D_RECT_F {
        left: (r.left - origin.0) as f32,
        top: (r.top - origin.1) as f32,
        right: (r.right - origin.0) as f32,
        bottom: (r.bottom - origin.1) as f32,
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RenderStyle {
    pub origin: (f64, f64),
    pub scale: f32,
    pub show_selection_chrome: bool,
}

pub(crate) fn draw_annotations(
    context: &ID2D1DeviceContext,
    resources: &RenderResources,
    selection: Rect,
    store: &AnnotationStore,
    draft: Option<&AnnotationKind>,
    style: RenderStyle,
) -> Result<()> {
    let RenderStyle {
        origin,
        scale,
        show_selection_chrome,
    } = style;
    // SAFETY: Caller owns an active BeginDraw/EndDraw session on `context`.
    unsafe {
        let factory: ID2D1Factory = context.GetFactory()?;
        let clip_rect = d2d_rect(selection, origin);
        context.PushAxisAlignedClip(&clip_rect, D2D1_ANTIALIAS_MODE_ALIASED);

        // Layer 1: Spotlight veil (Selection \ Union(Spotlights)) beneath vector strokes.
        let spotlights: Vec<Rect> = store
            .items
            .iter()
            .map(|item| &item.kind)
            .chain(draft)
            .filter_map(|kind| match kind {
                AnnotationKind::Spotlight { rect } => Some(*rect),
                _ => None,
            })
            .collect();

        if !spotlights.is_empty() {
            let sel_geom = factory.CreateRectangleGeometry(&clip_rect)?;
            let mut union_geom: Option<ID2D1Geometry> = None;
            for spot in &spotlights {
                let spot_geom: ID2D1Geometry = factory
                    .CreateRectangleGeometry(&d2d_rect(*spot, origin))?
                    .cast()?;
                union_geom = Some(match union_geom {
                    None => spot_geom,
                    Some(prev) => {
                        let path = factory.CreatePathGeometry()?;
                        let sink = path.Open()?;
                        prev.CombineWithGeometry(
                            &spot_geom,
                            D2D1_COMBINE_MODE_UNION,
                            None,
                            0.25,
                            &sink,
                        )?;
                        sink.Close()?;
                        path.cast()?
                    }
                });
            }
            if let Some(union_geom) = union_geom {
                let veil = factory.CreatePathGeometry()?;
                let sink = veil.Open()?;
                sel_geom.CombineWithGeometry(
                    &union_geom,
                    D2D1_COMBINE_MODE_EXCLUDE,
                    None,
                    0.25,
                    &sink,
                )?;
                sink.Close()?;
                let veil_brush = context.CreateSolidColorBrush(
                    &D2D1_COLOR_F {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: store.spotlight_dim.veil_alpha(resources.is_export),
                    },
                    None,
                )?;
                context.FillGeometry(&veil, &veil_brush, None);
            }

            if !resources.is_export {
                let inset = 0.5 * scale;
                for spot in &spotlights {
                    let mut r = d2d_rect(*spot, origin);
                    r.left += inset;
                    r.top += inset;
                    r.right -= inset;
                    r.bottom -= inset;
                    if r.right > r.left && r.bottom > r.top {
                        context.DrawRectangle(&r, &resources.spotlight_guide, scale, None);
                    }
                }
            }
        }

        // Layer 2: Vector strokes (Rect, Arrow, Freehand).
        for kind in store.items.iter().map(|i| &i.kind).chain(draft) {
            draw_vector_kind(context, &factory, resources, kind, origin, scale)?;
        }

        context.PopAxisAlignedClip();

        // Layer 3: Interactive selection outline & edit handles (preview only).
        if show_selection_chrome && let Some(selected) = store.selected() {
            draw_selection_chrome(context, resources, selected, origin, scale)?;
        }
        Ok(())
    }
}

unsafe fn draw_vector_kind(
    context: &ID2D1DeviceContext,
    factory: &ID2D1Factory,
    resources: &RenderResources,
    kind: &AnnotationKind,
    origin: (f64, f64),
    scale: f32,
) -> Result<()> {
    // SAFETY: Caller owns active D2D1 Draw session and factory.
    unsafe {
        match kind {
            AnnotationKind::Spotlight { .. } => {}
            AnnotationKind::Rect { rect, color, width } => {
                let brush = resources.ink_brush(*color);
                let stroke = width.dip() * scale;
                let r = d2d_rect(*rect, origin);
                context.DrawRectangle(&r, brush, stroke, Some(&resources.round_stroke));
            }
            AnnotationKind::Arrow {
                start,
                end,
                color,
                width,
            } => {
                let sx = (start.0 - origin.0) as f32;
                let sy = (start.1 - origin.1) as f32;
                let ex = (end.0 - origin.0) as f32;
                let ey = (end.1 - origin.1) as f32;
                let dx = ex - sx;
                let dy = ey - sy;
                let len = (dx * dx + dy * dy).sqrt();
                if len < 0.5 {
                    return Ok(());
                }
                let brush = resources.ink_brush(*color);
                let stroke = width.dip() * scale;
                let ux = dx / len;
                let uy = dy / len;
                let px = -uy;
                let py = ux;
                let head_len = (stroke * 4.2).max(12.0 * scale).min(len * 0.65);
                let head_half_w = head_len * 0.44;
                let shaft_end_x = ex - ux * (head_len * 0.65);
                let shaft_end_y = ey - uy * (head_len * 0.65);

                if len > head_len * 0.72 {
                    context.DrawLine(
                        Vector2 { X: sx, Y: sy },
                        Vector2 {
                            X: shaft_end_x,
                            Y: shaft_end_y,
                        },
                        brush,
                        stroke,
                        Some(&resources.round_stroke),
                    );
                }

                let notch_x = ex - ux * (head_len * 0.80);
                let notch_y = ey - uy * (head_len * 0.80);
                let base_x = ex - ux * head_len;
                let base_y = ey - uy * head_len;

                let path = factory.CreatePathGeometry()?;
                let sink = path.Open()?;
                sink.BeginFigure(Vector2 { X: ex, Y: ey }, D2D1_FIGURE_BEGIN_FILLED);
                sink.AddLines(&[
                    Vector2 {
                        X: base_x + px * head_half_w,
                        Y: base_y + py * head_half_w,
                    },
                    Vector2 {
                        X: notch_x,
                        Y: notch_y,
                    },
                    Vector2 {
                        X: base_x - px * head_half_w,
                        Y: base_y - py * head_half_w,
                    },
                ]);
                sink.EndFigure(D2D1_FIGURE_END_CLOSED);
                sink.Close()?;
                context.FillGeometry(&path, brush, None);
            }
            AnnotationKind::Freehand {
                points,
                color,
                width,
            } => {
                if points.len() < 2 {
                    return Ok(());
                }
                let brush = resources.ink_brush(*color);
                let stroke = width.dip() * scale;
                let path = factory.CreatePathGeometry()?;
                let sink = path.Open()?;
                let first = Vector2 {
                    X: (points[0].0 - origin.0) as f32,
                    Y: (points[0].1 - origin.1) as f32,
                };
                sink.BeginFigure(first, D2D1_FIGURE_BEGIN_HOLLOW);
                if points.len() == 2 {
                    sink.AddLine(Vector2 {
                        X: (points[1].0 - origin.0) as f32,
                        Y: (points[1].1 - origin.1) as f32,
                    });
                } else {
                    for i in 1..points.len() - 1 {
                        let cx = (points[i].0 - origin.0) as f32;
                        let cy = (points[i].1 - origin.1) as f32;
                        let nx = (points[i + 1].0 - origin.0) as f32;
                        let ny = (points[i + 1].1 - origin.1) as f32;
                        sink.AddQuadraticBezier(&D2D1_QUADRATIC_BEZIER_SEGMENT {
                            point1: Vector2 { X: cx, Y: cy },
                            point2: Vector2 {
                                X: (cx + nx) * 0.5,
                                Y: (cy + ny) * 0.5,
                            },
                        });
                    }
                    let last = points.last().expect("non-empty points");
                    sink.AddLine(Vector2 {
                        X: (last.0 - origin.0) as f32,
                        Y: (last.1 - origin.1) as f32,
                    });
                }
                sink.EndFigure(D2D1_FIGURE_END_OPEN);
                sink.Close()?;
                context.DrawGeometry(&path, brush, stroke, Some(&resources.round_stroke));
            }
        }
        Ok(())
    }
}

unsafe fn draw_selection_chrome(
    context: &ID2D1DeviceContext,
    resources: &RenderResources,
    annotation: &Annotation,
    origin: (f64, f64),
    scale: f32,
) -> Result<()> {
    // SAFETY: Caller owns active D2D1 Draw session and cached theme brushes.
    unsafe {
        let bounds = annotation.kind.bounds();
        let pad = 4.0 * scale;
        let box_rect = D2D_RECT_F {
            left: (bounds.left - origin.0) as f32 - pad,
            top: (bounds.top - origin.1) as f32 - pad,
            right: (bounds.right - origin.0) as f32 + pad,
            bottom: (bounds.bottom - origin.1) as f32 + pad,
        };
        context.DrawRectangle(
            &box_rect,
            &resources.accent_base,
            scale,
            Some(&resources.dashed_stroke),
        );

        let r = 3.5 * scale;
        for (_, pos) in annotation.kind.handles() {
            let x = (pos.0 - origin.0) as f32;
            let y = (pos.1 - origin.1) as f32;
            let ellipse = D2D1_ELLIPSE {
                point: Vector2 { X: x, Y: y },
                radiusX: r,
                radiusY: r,
            };
            context.FillEllipse(&ellipse, &resources.text_primary);
            context.DrawEllipse(&ellipse, &resources.accent_base, 1.25 * scale, None);
        }
        Ok(())
    }
}

/// Composite annotations on top of the already Visual-Match-converted sRGB RGBA8 crop.
pub(crate) fn composite_export_rgba8(
    device: &Device,
    rgba8: &[u8],
    crop: Crop,
    scale: f32,
    store: &AnnotationStore,
) -> Result<Vec<u8>> {
    if !store.has_annotations() {
        return Ok(rgba8.to_vec());
    }
    let expected_len = (crop.width as usize)
        .checked_mul(crop.height as usize)
        .and_then(|px| px.checked_mul(4))
        .ok_or_else(|| Error::new(E_FAIL, "Export image dimensions overflow"))?;
    if rgba8.len() != expected_len {
        return Err(Error::new(E_FAIL, "Export buffer size mismatch"));
    }

    // SAFETY: Device and immediate context are worker-owned; texture dimensions are validated.
    unsafe {
        let row_bytes = crop.width * 4;
        let render_desc = D3D11_TEXTURE2D_DESC {
            Width: crop.width,
            Height: crop.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE).0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut target_tex = None;
        device
            .native
            .CreateTexture2D(&render_desc, None, Some(&mut target_tex))?;
        let target_tex = target_tex.ok_or_else(|| Error::new(E_FAIL, "No sRGB export texture"))?;

        device
            .context
            .UpdateSubresource(&target_tex, 0, None, rgba8.as_ptr().cast(), row_bytes, 0);

        let dxgi: IDXGIDevice = device.native.cast()?;
        let surface: IDXGISurface = target_tex.cast()?;
        let d2d_device = D2D1CreateDevice(&dxgi, None)?;
        let context = d2d_device.CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
        let target_bitmap = context.CreateBitmapFromDxgiSurface(
            &surface,
            Some(&D2D1_BITMAP_PROPERTIES1 {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_R8G8B8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0,
                dpiY: 96.0,
                bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                ..Default::default()
            }),
        )?;
        context.SetTarget(&target_bitmap);
        context.SetDpi(96.0, 96.0);

        let resources = RenderResources::new(&context, 1.0, true)?;
        // Use exact pixel crop bounds so fractional Shift-resized selections never leave an undimmed hairline.
        let crop_bounds = Rect {
            left: f64::from(crop.x),
            top: f64::from(crop.y),
            right: f64::from(crop.x + crop.width),
            bottom: f64::from(crop.y + crop.height),
        };
        context.BeginDraw();
        let draw_result = draw_annotations(
            &context,
            &resources,
            crop_bounds,
            store,
            None,
            RenderStyle {
                origin: (f64::from(crop.x), f64::from(crop.y)),
                scale,
                show_selection_chrome: false,
            },
        );
        let end_result = context.EndDraw(None, None);
        context.SetTarget(None);
        draw_result?;
        end_result?;

        let staging_desc = D3D11_TEXTURE2D_DESC {
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..render_desc
        };
        let mut staging = None;
        device
            .native
            .CreateTexture2D(&staging_desc, None, Some(&mut staging))?;
        let staging = staging.ok_or_else(|| Error::new(E_FAIL, "No staging export texture"))?;
        device.context.CopyResource(&staging, &target_tex);

        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        device
            .context
            .Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        struct UnmapGuard<'a> {
            ctx: &'a ID3D11DeviceContext,
            tex: &'a ID3D11Texture2D,
        }
        impl Drop for UnmapGuard<'_> {
            fn drop(&mut self) {
                // SAFETY: Unmaps the single mapped subresource zero on drop.
                unsafe { self.ctx.Unmap(self.tex, 0) };
            }
        }
        let _guard = UnmapGuard {
            ctx: &device.context,
            tex: &staging,
        };
        if mapped.pData.is_null() || mapped.RowPitch < row_bytes {
            return Err(Error::new(E_FAIL, "Invalid mapped sRGB export stride"));
        }
        let mut output = vec![0u8; expected_len];
        for y in 0..crop.height as usize {
            std::ptr::copy_nonoverlapping(
                (mapped.pData as *const u8).add(y * mapped.RowPitch as usize),
                output.as_mut_ptr().add(y * row_bytes as usize),
                row_bytes as usize,
            );
        }
        Ok(output)
    }
}

pub(crate) fn hit_test_selected_handle(
    store: &AnnotationStore,
    point: (f64, f64),
    scale: f64,
) -> Option<(u64, AnnotationHandle)> {
    let selected = store.selected()?;
    selected
        .kind
        .hit_handle(point, 7.0 * scale)
        .map(|h| (selected.id, h))
}

pub(crate) fn merge_drag_rect(
    start: (f64, f64),
    end: (f64, f64),
    bounds: Rect,
    square: bool,
) -> Option<Rect> {
    let a = (
        start.0.clamp(bounds.left, bounds.right),
        start.1.clamp(bounds.top, bounds.bottom),
    );
    let clamped = (
        end.0.clamp(bounds.left, bounds.right),
        end.1.clamp(bounds.top, bounds.bottom),
    );
    let b = if square {
        let dx = clamped.0 - a.0;
        let dy = clamped.1 - a.1;
        let sx = if dx >= 0.0 { 1.0 } else { -1.0 };
        let sy = if dy >= 0.0 { 1.0 } else { -1.0 };
        let max_x = if sx > 0.0 {
            bounds.right - a.0
        } else {
            a.0 - bounds.left
        };
        let max_y = if sy > 0.0 {
            bounds.bottom - a.1
        } else {
            a.1 - bounds.top
        };
        let side = dx.abs().max(dy.abs()).min(max_x).min(max_y);
        (a.0 + sx * side, a.1 + sy * side)
    } else {
        clamped
    };
    if (a.0 - b.0).abs() < 0.5 || (a.1 - b.1).abs() < 0.5 {
        return None;
    }
    Some(Rect {
        left: a.0.min(b.0),
        top: a.1.min(b.1),
        right: a.0.max(b.0),
        bottom: a.1.max(b.1),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_and_spotlight_hit_order_work() {
        let mut store = AnnotationStore::default();
        let spot_id = store.add(AnnotationKind::Spotlight {
            rect: Rect {
                left: 50.0,
                top: 50.0,
                right: 300.0,
                bottom: 200.0,
            },
        });
        let arrow_id = store.add(AnnotationKind::Arrow {
            start: (100.0, 100.0),
            end: (220.0, 140.0),
            color: InkColor::Coral,
            width: StrokeWidth::Medium,
        });
        assert!(store.has_spotlights());
        assert_eq!(store.hit_test((160.0, 120.0), 1.0), Some(arrow_id));
        assert_eq!(store.hit_test((70.0, 180.0), 1.0), Some(spot_id));

        assert!(store.undo());
        assert_eq!(store.items.len(), 1);
        assert!(store.can_redo());
        assert!(store.redo());
        assert_eq!(store.items.len(), 2);

        store.selected_id = Some(spot_id);
        assert!(store.delete_selected());
        assert!(!store.has_spotlights());
    }

    #[test]
    fn shift_square_drag_stays_square_against_selection_edges() {
        let bounds = Rect {
            left: 100.0,
            top: 100.0,
            right: 250.0,
            bottom: 300.0,
        };
        let square = merge_drag_rect((150.0, 150.0), (400.0, 400.0), bounds, true).unwrap();
        assert!((square.width() - square.height()).abs() < 1e-6);
        assert_eq!(square.right, 250.0);
        assert_eq!(square.bottom, 250.0);
    }

    #[test]
    fn arrow_shift_snaps_to_45_degrees() {
        let snapped = constrain_arrow_angle((100.0, 100.0), (200.0, 112.0));
        assert!((snapped.1 - 100.0).abs() < 0.01);
        let diag = constrain_arrow_angle((100.0, 100.0), (190.0, 185.0));
        assert!(((diag.0 - 100.0) - (diag.1 - 100.0)).abs() < 0.01);
    }

    #[test]
    #[ignore = "Real D3D11/Direct2D GPU sRGB export compositing; run explicitly"]
    fn spotlight_and_vector_export_preserve_lit_pixels_and_dim_surroundings() {
        let device = Device::create().unwrap();
        let width = 80u32;
        let height = 60u32;
        let white_pixel = [240u8, 240u8, 240u8, 255u8];
        let input = white_pixel.repeat((width * height) as usize);

        let mut store = AnnotationStore::default();
        store.add(AnnotationKind::Spotlight {
            rect: Rect {
                left: 20.0,
                top: 15.0,
                right: 50.0,
                bottom: 45.0,
            },
        });
        store.add(AnnotationKind::Rect {
            rect: Rect {
                left: 10.0,
                top: 10.0,
                right: 70.0,
                bottom: 50.0,
            },
            color: InkColor::Coral,
            width: StrokeWidth::Small,
        });

        let out = composite_export_rgba8(
            &device,
            &input,
            Crop {
                x: 0,
                y: 0,
                width,
                height,
            },
            1.0,
            &store,
        )
        .unwrap();

        let px = |x: u32, y: u32| -> [u8; 4] {
            let idx = ((y * width + x) * 4) as usize;
            [out[idx], out[idx + 1], out[idx + 2], out[idx + 3]]
        };
        assert_eq!(
            px(35, 30),
            white_pixel,
            "Inside Spotlight cutout must remain 100% untouched"
        );
        let dimmed = px(5, 5);
        assert!(
            dimmed[0] < 130 && dimmed[0] > 90,
            "Outside Spotlight must be dimmed evenly (got {:?})",
            dimmed
        );
        let stroke = px(10, 25);
        assert!(
            stroke[0] > 200 && stroke[1] < 120,
            "Vector Rect stroke must retain accurate Coral color (got {:?})",
            stroke
        );
    }
}
