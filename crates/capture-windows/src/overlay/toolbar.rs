use super::{
    annotations::{AnnotationKind, AnnotationStore, InkColor, SpotlightDim, StrokeWidth, Tool},
    geometry::{Rect, place_floating_bars},
    icons::{Icon, draw_icon},
    theme::RenderResources,
};
use windows::{
    Win32::Graphics::{
        Direct2D::{Common::*, *},
        DirectWrite::*,
    },
    core::Result,
};
use windows_numerics::Vector2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolbarHit {
    Tool(Tool),
    AttributeToggle,
    Undo,
    Redo,
    Cancel,
    Confirm,
    ColorSwatch(InkColor),
    StrokeWidth(StrokeWidth),
    SpotlightDim(SpotlightDim),
    ClearSpotlights,
    BarBody,
}

#[derive(Clone, Debug)]
pub(crate) struct ToolbarState {
    pub tool: Tool,
    pub color: InkColor,
    pub width: StrokeWidth,
    pub subbar_open: bool,
    pub hovered: Option<ToolbarHit>,
    pub pressed: Option<ToolbarHit>,
}

impl Default for ToolbarState {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            color: InkColor::Coral,
            width: StrokeWidth::Medium,
            subbar_open: false,
            hovered: None,
            pressed: None,
        }
    }
}

impl ToolbarState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn set_tool(&mut self, next: Tool) {
        if self.tool == next && next != Tool::Select {
            self.tool = Tool::Select;
            self.subbar_open = false;
        } else {
            self.tool = next;
            self.subbar_open = next != Tool::Select;
        }
    }

    pub fn is_spotlight_context(&self, store: &AnnotationStore) -> bool {
        self.tool == Tool::Spotlight
            || (self.tool == Tool::Select
                && store
                    .selected()
                    .is_some_and(|item| item.kind.is_spotlight()))
    }

    pub fn subbar_mode(&self, store: &AnnotationStore) -> Option<SubbarMode> {
        if !self.subbar_open {
            return None;
        }
        if self.is_spotlight_context(store) {
            Some(SubbarMode::Spotlight)
        } else {
            Some(SubbarMode::Stroke)
        }
    }

    pub fn active_color(&self, store: &AnnotationStore) -> InkColor {
        if self.tool == Tool::Select
            && let Some(selected) = store.selected()
        {
            match selected.kind {
                AnnotationKind::Rect { color, .. }
                | AnnotationKind::Arrow { color, .. }
                | AnnotationKind::Freehand { color, .. } => return color,
                AnnotationKind::Spotlight { .. } => {}
            }
        }
        self.color
    }

    pub fn active_width(&self, store: &AnnotationStore) -> StrokeWidth {
        if self.tool == Tool::Select
            && let Some(selected) = store.selected()
        {
            match selected.kind {
                AnnotationKind::Rect { width, .. }
                | AnnotationKind::Arrow { width, .. }
                | AnnotationKind::Freehand { width, .. } => return width,
                AnnotationKind::Spotlight { .. } => {}
            }
        }
        self.width
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubbarMode {
    Stroke,
    Spotlight,
}

#[derive(Clone, Debug)]
struct ItemSlot {
    hit: ToolbarHit,
    rect: Rect,
}

#[derive(Clone, Debug)]
pub(crate) struct ToolbarLayout {
    pub bar_rect: Rect,
    pub subbar_rect: Option<Rect>,
    pub subbar_mode: Option<SubbarMode>,
    items: Vec<ItemSlot>,
    dividers: Vec<(f32, f32, f32)>,
}

impl ToolbarLayout {
    pub fn compute(
        selection: Rect,
        bounds: (f64, f64),
        scale: f64,
        toolbar: &ToolbarState,
        store: &AnnotationStore,
    ) -> Self {
        let pad_x = 8.0 * scale;
        let pad_y = 5.0 * scale;
        let btn_w = 28.0 * scale;
        let btn_h = 28.0 * scale;
        let attr_w = 34.0 * scale;
        let confirm_w = 64.0 * scale;
        let gap = 2.0 * scale;
        let div_gap = 6.0 * scale;
        let bar_h = btn_h + pad_y * 2.0;

        // Exact main bar order: Select | Rect, Highlight, Arrow, Freehand | Attr, Undo, Redo | Cancel, Done
        let bar_w = pad_x * 2.0
            + btn_w * 8.0
            + attr_w
            + confirm_w
            + gap * 7.0
            + div_gap * 3.0
            + 3.0 * scale;

        let subbar_mode = toolbar.subbar_mode(store);
        let subbar_size = subbar_mode.map(|mode| match mode {
            SubbarMode::Stroke => (232.0 * scale, 32.0 * scale),
            SubbarMode::Spotlight => {
                if store.has_spotlights() {
                    (226.0 * scale, 32.0 * scale)
                } else {
                    (174.0 * scale, 32.0 * scale)
                }
            }
        });

        let placed = place_floating_bars(selection, (bar_w, bar_h), subbar_size, bounds, scale);
        let mut items = Vec::with_capacity(20);
        let mut dividers = Vec::with_capacity(4);

        let mut x = placed.bar.left + pad_x;
        let y = placed.bar.top + pad_y;

        let mut push_btn = |x_ref: &mut f64, hit: ToolbarHit, w: f64| {
            items.push(ItemSlot {
                hit,
                rect: Rect {
                    left: *x_ref,
                    top: y,
                    right: *x_ref + w,
                    bottom: y + btn_h,
                },
            });
            *x_ref += w;
        };
        let mut push_div = |x_ref: &mut f64| {
            *x_ref += div_gap * 0.5;
            dividers.push((
                *x_ref as f32,
                (y + 5.0 * scale) as f32,
                (y + btn_h - 5.0 * scale) as f32,
            ));
            *x_ref += 1.0 * scale + div_gap * 0.5;
        };

        push_btn(&mut x, ToolbarHit::Tool(Tool::Select), btn_w);
        push_div(&mut x);

        // Required tool order: 矩形, 高亮, 箭头, 画笔
        for (idx, tool) in [Tool::Rect, Tool::Spotlight, Tool::Arrow, Tool::Freehand]
            .into_iter()
            .enumerate()
        {
            if idx > 0 {
                x += gap;
            }
            push_btn(&mut x, ToolbarHit::Tool(tool), btn_w);
        }
        push_div(&mut x);

        push_btn(&mut x, ToolbarHit::AttributeToggle, attr_w);
        x += gap;
        push_btn(&mut x, ToolbarHit::Undo, btn_w);
        x += gap;
        push_btn(&mut x, ToolbarHit::Redo, btn_w);
        push_div(&mut x);

        push_btn(&mut x, ToolbarHit::Cancel, btn_w);
        x += gap * 2.0;
        push_btn(&mut x, ToolbarHit::Confirm, confirm_w);

        if let (Some(sub_rect), Some(mode)) = (placed.subbar, subbar_mode) {
            let spx = 8.0 * scale;
            let spy = 4.0 * scale;
            let sh = sub_rect.height() - spy * 2.0;
            let sy = sub_rect.top + spy;
            let mut sx = sub_rect.left + spx;

            match mode {
                SubbarMode::Stroke => {
                    let sw = 20.0 * scale;
                    for (idx, color) in InkColor::ALL.into_iter().enumerate() {
                        if idx > 0 {
                            sx += 2.0 * scale;
                        }
                        items.push(ItemSlot {
                            hit: ToolbarHit::ColorSwatch(color),
                            rect: Rect {
                                left: sx,
                                top: sy,
                                right: sx + sw,
                                bottom: sy + sh,
                            },
                        });
                        sx += sw;
                    }
                    sx += 6.0 * scale;
                    dividers.push((
                        sx as f32,
                        (sy + 4.0 * scale) as f32,
                        (sy + sh - 4.0 * scale) as f32,
                    ));
                    sx += 7.0 * scale;
                    let ww = 22.0 * scale;
                    for (idx, weight) in StrokeWidth::ALL.into_iter().enumerate() {
                        if idx > 0 {
                            sx += 2.0 * scale;
                        }
                        items.push(ItemSlot {
                            hit: ToolbarHit::StrokeWidth(weight),
                            rect: Rect {
                                left: sx,
                                top: sy,
                                right: sx + ww,
                                bottom: sy + sh,
                            },
                        });
                        sx += ww;
                    }
                }
                SubbarMode::Spotlight => {
                    sx += 30.0 * scale;
                    let dw = 40.0 * scale;
                    for (idx, dim) in SpotlightDim::ALL.into_iter().enumerate() {
                        if idx > 0 {
                            sx += 3.0 * scale;
                        }
                        items.push(ItemSlot {
                            hit: ToolbarHit::SpotlightDim(dim),
                            rect: Rect {
                                left: sx,
                                top: sy,
                                right: sx + dw,
                                bottom: sy + sh,
                            },
                        });
                        sx += dw;
                    }
                    if store.has_spotlights() {
                        sx += 6.0 * scale;
                        dividers.push((
                            sx as f32,
                            (sy + 4.0 * scale) as f32,
                            (sy + sh - 4.0 * scale) as f32,
                        ));
                        sx += 7.0 * scale;
                        items.push(ItemSlot {
                            hit: ToolbarHit::ClearSpotlights,
                            rect: Rect {
                                left: sx,
                                top: sy,
                                right: sub_rect.right - spx,
                                bottom: sy + sh,
                            },
                        });
                    }
                }
            }
        }

        Self {
            bar_rect: placed.bar,
            subbar_rect: placed.subbar,
            subbar_mode,
            items,
            dividers,
        }
    }

    pub fn hit_test(&self, point: (f64, f64)) -> Option<ToolbarHit> {
        for slot in &self.items {
            if slot.rect.contains_inclusive(point) {
                return Some(slot.hit);
            }
        }
        if self.bar_rect.contains_inclusive(point)
            || self
                .subbar_rect
                .is_some_and(|r| r.contains_inclusive(point))
        {
            return Some(ToolbarHit::BarBody);
        }
        None
    }

    pub unsafe fn draw(
        &self,
        context: &ID2D1DeviceContext,
        resources: &RenderResources,
        text_factory: &IDWriteFactory,
        label_format: &IDWriteTextFormat,
        (toolbar, store, scale): (&ToolbarState, &AnnotationStore, f32),
    ) -> Result<()> {
        // SAFETY: Caller owns an active BeginDraw/EndDraw session on `context`.
        unsafe {
            let factory: ID2D1Factory = context.GetFactory()?;
            let bar_r = D2D1_ROUNDED_RECT {
                rect: to_d2d_rect(self.bar_rect),
                radiusX: 8.0 * scale,
                radiusY: 8.0 * scale,
            };
            context.FillRoundedRectangle(&bar_r, &resources.bar_bg);
            context.DrawRoundedRectangle(&bar_r, &resources.border_default, scale, None);

            if let Some(sub_rect) = self.subbar_rect {
                let sub_r = D2D1_ROUNDED_RECT {
                    rect: to_d2d_rect(sub_rect),
                    radiusX: 7.0 * scale,
                    radiusY: 7.0 * scale,
                };
                context.FillRoundedRectangle(&sub_r, &resources.subbar_bg);
                context.DrawRoundedRectangle(&sub_r, &resources.border_default, scale, None);
                if self.subbar_mode == Some(SubbarMode::Spotlight) {
                    draw_centered_text(
                        context,
                        text_factory,
                        label_format,
                        "Dim",
                        Rect {
                            left: sub_rect.left + 8.0 * f64::from(scale),
                            top: sub_rect.top,
                            right: sub_rect.left + 36.0 * f64::from(scale),
                            bottom: sub_rect.bottom,
                        },
                        &resources.text_secondary,
                    )?;
                }
            }

            for &(dx, dy1, dy2) in &self.dividers {
                context.DrawLine(
                    Vector2 { X: dx, Y: dy1 },
                    Vector2 { X: dx, Y: dy2 },
                    &resources.border_subtle,
                    scale,
                    None,
                );
            }

            let active_color = toolbar.active_color(store);
            let active_width = toolbar.active_width(store);
            let spotlight_ctx = toolbar.is_spotlight_context(store);

            for slot in &self.items {
                let is_hovered = toolbar.hovered == Some(slot.hit);
                let is_pressed = toolbar.pressed == Some(slot.hit);
                let center = (
                    ((slot.rect.left + slot.rect.right) * 0.5) as f32,
                    ((slot.rect.top + slot.rect.bottom) * 0.5) as f32,
                );
                let pill = D2D1_ROUNDED_RECT {
                    rect: to_d2d_rect(slot.rect),
                    radiusX: 6.0 * scale,
                    radiusY: 6.0 * scale,
                };

                match slot.hit {
                    ToolbarHit::Tool(tool) => {
                        let active = toolbar.tool == tool;
                        if active {
                            context.FillRoundedRectangle(&pill, &resources.accent_tint);
                            context.DrawRoundedRectangle(
                                &pill,
                                &resources.accent_border,
                                scale,
                                None,
                            );
                        } else if is_pressed || is_hovered {
                            context.FillRoundedRectangle(&pill, &resources.surface_hover);
                        }
                        let icon = match tool {
                            Tool::Select => Icon::Select,
                            Tool::Rect => Icon::Rect,
                            Tool::Spotlight => Icon::Spotlight,
                            Tool::Arrow => Icon::Arrow,
                            Tool::Freehand => Icon::Freehand,
                        };
                        let icon_brush = if active {
                            &resources.accent_base
                        } else if is_hovered {
                            &resources.text_primary
                        } else {
                            &resources.text_secondary
                        };
                        draw_icon(
                            context,
                            &factory,
                            &resources.round_stroke,
                            icon,
                            center,
                            scale,
                            icon_brush,
                        )?;
                    }
                    ToolbarHit::AttributeToggle => {
                        let open = toolbar.subbar_mode(store).is_some();
                        if open || is_hovered || is_pressed {
                            context.FillRoundedRectangle(&pill, &resources.surface_hover);
                        }
                        if spotlight_ctx {
                            draw_centered_text(
                                context,
                                text_factory,
                                label_format,
                                store.spotlight_dim.label(),
                                slot.rect,
                                if open {
                                    &resources.accent_base
                                } else {
                                    &resources.text_primary
                                },
                            )?;
                        } else {
                            let dot_x = center.0 - 4.5 * scale;
                            let ell = D2D1_ELLIPSE {
                                point: Vector2 {
                                    X: dot_x,
                                    Y: center.1,
                                },
                                radiusX: 5.0 * scale,
                                radiusY: 5.0 * scale,
                            };
                            context.FillEllipse(&ell, resources.ink_brush(active_color));
                            context.DrawEllipse(&ell, &resources.border_default, scale, None);
                            let weight_r = match active_width {
                                StrokeWidth::Small => 1.5 * scale,
                                StrokeWidth::Medium => 2.3 * scale,
                                StrokeWidth::Large => 3.3 * scale,
                            };
                            context.FillEllipse(
                                &D2D1_ELLIPSE {
                                    point: Vector2 {
                                        X: center.0 + 7.0 * scale,
                                        Y: center.1,
                                    },
                                    radiusX: weight_r,
                                    radiusY: weight_r,
                                },
                                &resources.text_primary,
                            );
                        }
                    }
                    ToolbarHit::Undo => {
                        let enabled = store.can_undo();
                        if enabled && (is_hovered || is_pressed) {
                            context.FillRoundedRectangle(&pill, &resources.surface_hover);
                        }
                        draw_icon(
                            context,
                            &factory,
                            &resources.round_stroke,
                            Icon::Undo,
                            center,
                            scale,
                            if enabled {
                                if is_hovered {
                                    &resources.text_primary
                                } else {
                                    &resources.text_secondary
                                }
                            } else {
                                &resources.text_disabled
                            },
                        )?;
                    }
                    ToolbarHit::Redo => {
                        let enabled = store.can_redo();
                        if enabled && (is_hovered || is_pressed) {
                            context.FillRoundedRectangle(&pill, &resources.surface_hover);
                        }
                        draw_icon(
                            context,
                            &factory,
                            &resources.round_stroke,
                            Icon::Redo,
                            center,
                            scale,
                            if enabled {
                                if is_hovered {
                                    &resources.text_primary
                                } else {
                                    &resources.text_secondary
                                }
                            } else {
                                &resources.text_disabled
                            },
                        )?;
                    }
                    ToolbarHit::Cancel => {
                        if is_hovered || is_pressed {
                            context.FillRoundedRectangle(&pill, &resources.surface_hover);
                        }
                        draw_icon(
                            context,
                            &factory,
                            &resources.round_stroke,
                            Icon::Cancel,
                            center,
                            scale,
                            if is_hovered {
                                &resources.text_primary
                            } else {
                                &resources.text_secondary
                            },
                        )?;
                    }
                    ToolbarHit::Confirm => {
                        let fill = if is_pressed {
                            &resources.accent_press
                        } else if is_hovered {
                            &resources.accent_hover
                        } else {
                            &resources.accent_base
                        };
                        context.FillRoundedRectangle(&pill, fill);
                        draw_icon(
                            context,
                            &factory,
                            &resources.round_stroke,
                            Icon::Confirm,
                            (slot.rect.left as f32 + 15.0 * scale, center.1),
                            scale * 0.92,
                            &resources.text_on_accent,
                        )?;
                        draw_centered_text(
                            context,
                            text_factory,
                            label_format,
                            "Done",
                            Rect {
                                left: slot.rect.left + 21.0 * f64::from(scale),
                                top: slot.rect.top,
                                right: slot.rect.right - 5.0 * f64::from(scale),
                                bottom: slot.rect.bottom,
                            },
                            &resources.text_on_accent,
                        )?;
                    }
                    ToolbarHit::ColorSwatch(ink) => {
                        let active = active_color == ink;
                        if is_hovered && !active {
                            context.FillRoundedRectangle(&pill, &resources.surface_hover);
                        }
                        let ell = D2D1_ELLIPSE {
                            point: Vector2 {
                                X: center.0,
                                Y: center.1,
                            },
                            radiusX: 5.5 * scale,
                            radiusY: 5.5 * scale,
                        };
                        context.FillEllipse(&ell, resources.ink_brush(ink));
                        if active {
                            context.DrawEllipse(
                                &D2D1_ELLIPSE {
                                    point: ell.point,
                                    radiusX: 7.5 * scale,
                                    radiusY: 7.5 * scale,
                                },
                                &resources.text_primary,
                                1.3 * scale,
                                None,
                            );
                        } else {
                            context.DrawEllipse(&ell, &resources.border_default, 0.8 * scale, None);
                        }
                    }
                    ToolbarHit::StrokeWidth(weight) => {
                        let active = active_width == weight;
                        if active {
                            context.FillRoundedRectangle(&pill, &resources.accent_tint);
                            context.DrawRoundedRectangle(
                                &pill,
                                &resources.accent_border,
                                scale,
                                None,
                            );
                        } else if is_hovered {
                            context.FillRoundedRectangle(&pill, &resources.surface_hover);
                        }
                        let r = match weight {
                            StrokeWidth::Small => 1.6 * scale,
                            StrokeWidth::Medium => 2.7 * scale,
                            StrokeWidth::Large => 4.2 * scale,
                        };
                        context.FillEllipse(
                            &D2D1_ELLIPSE {
                                point: Vector2 {
                                    X: center.0,
                                    Y: center.1,
                                },
                                radiusX: r,
                                radiusY: r,
                            },
                            if active {
                                &resources.accent_base
                            } else {
                                &resources.text_primary
                            },
                        );
                    }
                    ToolbarHit::SpotlightDim(dim) => {
                        let active = store.spotlight_dim == dim;
                        if active {
                            context.FillRoundedRectangle(&pill, &resources.accent_tint);
                            context.DrawRoundedRectangle(
                                &pill,
                                &resources.accent_border,
                                scale,
                                None,
                            );
                        } else if is_hovered {
                            context.FillRoundedRectangle(&pill, &resources.surface_hover);
                        }
                        draw_centered_text(
                            context,
                            text_factory,
                            label_format,
                            dim.label(),
                            slot.rect,
                            if active {
                                &resources.accent_base
                            } else {
                                &resources.text_secondary
                            },
                        )?;
                    }
                    ToolbarHit::ClearSpotlights => {
                        if is_hovered || is_pressed {
                            context.FillRoundedRectangle(&pill, &resources.surface_hover);
                        }
                        draw_centered_text(
                            context,
                            text_factory,
                            label_format,
                            "Clear",
                            slot.rect,
                            if is_hovered {
                                &resources.text_primary
                            } else {
                                &resources.text_secondary
                            },
                        )?;
                    }
                    ToolbarHit::BarBody => {}
                }
            }
            Ok(())
        }
    }
}

unsafe fn draw_centered_text(
    context: &ID2D1DeviceContext,
    factory: &IDWriteFactory,
    format: &IDWriteTextFormat,
    text: &str,
    rect: Rect,
    brush: &ID2D1SolidColorBrush,
) -> Result<()> {
    // SAFETY: Caller owns active D2D1 session and DirectWrite factory/format.
    unsafe {
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let max_w = rect.width().max(1.0) as f32;
        let max_h = rect.height().max(1.0) as f32;
        let layout = factory.CreateTextLayout(&utf16, format, max_w, max_h)?;
        layout.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
        layout.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        context.DrawTextLayout(
            Vector2 {
                X: rect.left as f32,
                Y: rect.top as f32,
            },
            &layout,
            brush,
            D2D1_DRAW_TEXT_OPTIONS_NONE,
        );
        Ok(())
    }
}

fn to_d2d_rect(r: Rect) -> D2D_RECT_F {
    D2D_RECT_F {
        left: r.left as f32,
        top: r.top as f32,
        right: r.right as f32,
        bottom: r.bottom as f32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toolbar_places_required_tools_in_exact_order() {
        let state = ToolbarState::default();
        let store = AnnotationStore::default();
        let layout = ToolbarLayout::compute(
            Rect {
                left: 100.0,
                top: 100.0,
                right: 700.0,
                bottom: 450.0,
            },
            (1920.0, 1080.0),
            1.0,
            &state,
            &store,
        );
        let tools: Vec<Tool> = layout
            .items
            .iter()
            .filter_map(|slot| match slot.hit {
                ToolbarHit::Tool(t) => Some(t),
                _ => None,
            })
            .collect();
        assert_eq!(
            tools,
            vec![
                Tool::Select,
                Tool::Rect,
                Tool::Spotlight,
                Tool::Arrow,
                Tool::Freehand,
            ]
        );
        let xs: Vec<f64> = layout
            .items
            .iter()
            .filter_map(|slot| match slot.hit {
                ToolbarHit::Tool(_) => Some(slot.rect.left),
                _ => None,
            })
            .collect();
        assert!(xs.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn subbar_can_collapse_even_in_drawing_tool() {
        let mut state = ToolbarState::default();
        let store = AnnotationStore::default();
        state.set_tool(Tool::Spotlight);
        assert_eq!(state.subbar_mode(&store), Some(SubbarMode::Spotlight));
        state.subbar_open = !state.subbar_open;
        assert_eq!(state.subbar_mode(&store), None);
    }
}
