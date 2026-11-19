#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Rect {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Crop {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct FloatingBarPlacement {
    pub bar: Rect,
    pub subbar: Option<Rect>,
    pub inside_selection: bool,
}

impl Rect {
    pub fn width(self) -> f64 {
        (self.right - self.left).max(0.0)
    }

    pub fn height(self) -> f64 {
        (self.bottom - self.top).max(0.0)
    }

    pub fn min_dimensions(scale: f64) -> (f64, f64) {
        (32.0 * scale, 24.0 * scale)
    }

    pub fn contains(self, point: (f64, f64)) -> bool {
        point.0 >= self.left && point.0 < self.right && point.1 >= self.top && point.1 < self.bottom
    }

    pub fn contains_inclusive(self, point: (f64, f64)) -> bool {
        point.0 >= self.left
            && point.0 <= self.right
            && point.1 >= self.top
            && point.1 <= self.bottom
    }

    pub fn is_valid_selection(self, scale: f64) -> bool {
        let (min_w, min_h) = Self::min_dimensions(scale);
        self.right - self.left >= min_w && self.bottom - self.top >= min_h
    }

    pub fn drag(start: (f64, f64), end: (f64, f64), width: f64, height: f64) -> Option<Self> {
        if ![start.0, start.1, end.0, end.1, width, height]
            .iter()
            .all(|v| v.is_finite())
            || width <= 0.0
            || height <= 0.0
        {
            return None;
        }
        let a = (start.0.clamp(0.0, width), start.1.clamp(0.0, height));
        let b = (end.0.clamp(0.0, width), end.1.clamp(0.0, height));
        if a.0 == b.0 || a.1 == b.1 {
            return None;
        }
        Some(Self {
            left: a.0.min(b.0),
            top: a.1.min(b.1),
            right: a.0.max(b.0),
            bottom: a.1.max(b.1),
        })
    }

    pub fn handle_points(self) -> [(Handle, (f64, f64)); 8] {
        let cx = (self.left + self.right) * 0.5;
        let cy = (self.top + self.bottom) * 0.5;
        // Corners precede edges so tight selections prioritize diagonal handles.
        [
            (Handle::TopLeft, (self.left, self.top)),
            (Handle::TopRight, (self.right, self.top)),
            (Handle::BottomRight, (self.right, self.bottom)),
            (Handle::BottomLeft, (self.left, self.bottom)),
            (Handle::Top, (cx, self.top)),
            (Handle::Right, (self.right, cy)),
            (Handle::Bottom, (cx, self.bottom)),
            (Handle::Left, (self.left, cy)),
        ]
    }

    pub fn hit_handle(self, point: (f64, f64), radius: f64) -> Option<Handle> {
        self.handle_points().into_iter().find_map(|(handle, pos)| {
            ((point.0 - pos.0).abs() <= radius && (point.1 - pos.1).abs() <= radius)
                .then_some(handle)
        })
    }

    pub fn translate(self, delta: (f64, f64), bounds: (f64, f64)) -> Self {
        let w = self.width().min(bounds.0);
        let h = self.height().min(bounds.1);
        let left = (self.left + delta.0).clamp(0.0, (bounds.0 - w).max(0.0));
        let top = (self.top + delta.1).clamp(0.0, (bounds.1 - h).max(0.0));
        Self {
            left,
            top,
            right: left + w,
            bottom: top + h,
        }
    }

    pub fn resize_with_handle(
        self,
        handle: Handle,
        pointer: (f64, f64),
        bounds: (f64, f64),
        scale: f64,
        keep_aspect: bool,
    ) -> Self {
        let (min_w, min_h) = Self::min_dimensions(scale);
        let px = pointer.0.clamp(0.0, bounds.0);
        let py = pointer.1.clamp(0.0, bounds.1);

        let mut left = self.left;
        let mut top = self.top;
        let mut right = self.right;
        let mut bottom = self.bottom;

        match handle {
            Handle::TopLeft => {
                left = px.min(right - min_w);
                top = py.min(bottom - min_h);
            }
            Handle::Top => {
                top = py.min(bottom - min_h);
            }
            Handle::TopRight => {
                right = px.max(left + min_w);
                top = py.min(bottom - min_h);
            }
            Handle::Right => {
                right = px.max(left + min_w);
            }
            Handle::BottomRight => {
                right = px.max(left + min_w);
                bottom = py.max(top + min_h);
            }
            Handle::Bottom => {
                bottom = py.max(top + min_h);
            }
            Handle::BottomLeft => {
                left = px.min(right - min_w);
                bottom = py.max(top + min_h);
            }
            Handle::Left => {
                left = px.min(right - min_w);
            }
        }

        if keep_aspect
            && self.width() > 0.0
            && self.height() > 0.0
            && matches!(
                handle,
                Handle::TopLeft | Handle::TopRight | Handle::BottomRight | Handle::BottomLeft
            )
        {
            let aspect = self.width() / self.height();
            let (anchor_x, anchor_y, dir_x, dir_y) = match handle {
                Handle::TopLeft => (right, bottom, -1.0, -1.0),
                Handle::TopRight => (left, bottom, 1.0, -1.0),
                Handle::BottomRight => (left, top, 1.0, 1.0),
                Handle::BottomLeft => (right, top, -1.0, 1.0),
                _ => unreachable!(),
            };
            let max_w = if dir_x < 0.0 {
                anchor_x
            } else {
                bounds.0 - anchor_x
            };
            let max_h = if dir_y < 0.0 {
                anchor_y
            } else {
                bounds.1 - anchor_y
            };
            let mut w = (right - left).max(min_w);
            let mut h = (bottom - top).max(min_h);
            if w / h > aspect {
                h = (w / aspect).max(min_h);
                w = h * aspect;
            } else {
                w = (h * aspect).max(min_w);
                h = w / aspect;
            }
            if w > max_w {
                w = max_w;
                h = (w / aspect).max(min_h);
            }
            if h > max_h {
                h = max_h;
                w = (h * aspect).max(min_w);
            }
            let target_x = (anchor_x + dir_x * w).clamp(0.0, bounds.0);
            let target_y = (anchor_y + dir_y * h).clamp(0.0, bounds.1);
            left = anchor_x.min(target_x);
            right = anchor_x.max(target_x);
            top = anchor_y.min(target_y);
            bottom = anchor_y.max(target_y);
        }

        Self {
            left: left.clamp(0.0, bounds.0),
            top: top.clamp(0.0, bounds.1),
            right: right.clamp(0.0, bounds.0),
            bottom: bottom.clamp(0.0, bounds.1),
        }
    }

    pub fn crop(self, logical: (f64, f64), pixels: (u32, u32)) -> Option<Crop> {
        if ![
            self.left,
            self.top,
            self.right,
            self.bottom,
            logical.0,
            logical.1,
        ]
        .iter()
        .all(|v| v.is_finite())
            || logical.0 <= 0.0
            || logical.1 <= 0.0
            || self.left < 0.0
            || self.top < 0.0
            || self.right > logical.0 + 0.000001
            || self.bottom > logical.1 + 0.000001
            || self.right <= self.left
            || self.bottom <= self.top
            || pixels.0 == 0
            || pixels.1 == 0
        {
            return None;
        }
        let x = (self.left * pixels.0 as f64 / logical.0)
            .floor()
            .clamp(0.0, pixels.0 as f64) as u32;
        let y = (self.top * pixels.1 as f64 / logical.1)
            .floor()
            .clamp(0.0, pixels.1 as f64) as u32;
        if x >= pixels.0 || y >= pixels.1 {
            return None;
        }
        let right = (self.right * pixels.0 as f64 / logical.0)
            .ceil()
            .clamp((x + 1) as f64, pixels.0 as f64) as u32;
        let bottom = (self.bottom * pixels.1 as f64 / logical.1)
            .ceil()
            .clamp((y + 1) as f64, pixels.1 as f64) as u32;
        Some(Crop {
            x,
            y,
            width: right - x,
            height: bottom - y,
        })
    }
}

pub(crate) fn place_floating_bars(
    selection: Rect,
    bar_size: (f64, f64),
    subbar_size: Option<(f64, f64)>,
    bounds: (f64, f64),
    scale: f64,
) -> FloatingBarPlacement {
    let margin = 12.0 * scale;
    let gap = 10.0 * scale;
    let sub_gap = 6.0 * scale;
    let sub_h = subbar_size.map_or(0.0, |(_, h)| h + sub_gap);
    let total_h = bar_size.1 + sub_h;

    let bar_x = (selection.right - bar_size.0)
        .max(selection.left)
        .clamp(margin, (bounds.0 - bar_size.0 - margin).max(margin));

    let (bar_y, subbar_y, inside_selection) =
        if selection.bottom + gap + total_h <= bounds.1 - margin {
            let by = selection.bottom + gap;
            (by, Some(by + bar_size.1 + sub_gap), false)
        } else if selection.top - gap - total_h >= margin {
            let by = selection.top - gap - bar_size.1;
            (by, subbar_size.map(|(_, h)| by - sub_gap - h), false)
        } else {
            // Fallback: float near the bottom inside the selection so tall/fullscreen captures stay reachable.
            let by = (selection.bottom - margin - bar_size.1)
                .clamp(margin, (bounds.1 - bar_size.1 - margin).max(margin));
            (
                by,
                subbar_size.map(|(_, h)| (by - sub_gap - h).max(margin)),
                true,
            )
        };

    let subbar = subbar_size.zip(subbar_y).map(|((sw, sh), sy)| {
        let sx = (bar_x + bar_size.0 - sw).clamp(margin, (bounds.0 - sw - margin).max(margin));
        Rect {
            left: sx,
            top: sy,
            right: sx + sw,
            bottom: sy + sh,
        }
    });

    FloatingBarPlacement {
        bar: Rect {
            left: bar_x,
            top: bar_y,
            right: bar_x + bar_size.0,
            bottom: bar_y + bar_size.1,
        },
        subbar,
        inside_selection,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_minimum_matches_logical_size_at_scaled_dpi() {
        let scale = 1.5;
        let minimum = Rect::drag((0.0, 0.0), (48.0, 36.0), 100.0, 100.0).unwrap();
        assert!(minimum.is_valid_selection(scale));
        assert!(
            !Rect {
                right: 47.0,
                ..minimum
            }
            .is_valid_selection(scale)
        );
        assert!(
            !Rect {
                bottom: 35.0,
                ..minimum
            }
            .is_valid_selection(scale)
        );
    }

    #[test]
    fn reverse_drag_and_fractional_dpi_crop_round_outwards() {
        let rect = Rect::drag((110.25, 90.5), (10.1, 20.1), 2560.0, 1440.0).unwrap();
        assert_eq!(
            rect.crop((2560.0, 1440.0), (3840, 2160)),
            Some(Crop {
                x: 15,
                y: 30,
                width: 151,
                height: 106
            })
        );
        let full = Rect::drag((-10.0, -20.0), (3000.0, 1500.0), 2560.0, 1440.0).unwrap();
        assert_eq!(
            full.crop((2560.0, 1440.0), (3840, 2160)),
            Some(Crop {
                x: 0,
                y: 0,
                width: 3840,
                height: 2160
            })
        );
    }

    #[test]
    fn invalid_or_empty_geometry_cannot_crop_a_frame() {
        assert!(Rect::drag((f64::NAN, 0.0), (10.0, 10.0), 100.0, 100.0).is_none());
        assert!(Rect::drag((10.0, 10.0), (10.0, 20.0), 100.0, 100.0).is_none());
        let rect = Rect {
            left: 0.0,
            top: 0.0,
            right: 100.1,
            bottom: 100.0,
        };
        assert!(rect.crop((100.0, 100.0), (150, 150)).is_none());
    }

    #[test]
    fn handles_resize_translate_and_clamp_cleanly() {
        let rect = Rect {
            left: 100.0,
            top: 100.0,
            right: 300.0,
            bottom: 220.0,
        };
        assert_eq!(rect.hit_handle((102.0, 102.0), 7.0), Some(Handle::TopLeft));
        assert_eq!(rect.hit_handle((200.0, 220.0), 7.0), Some(Handle::Bottom));

        let shrunk =
            rect.resize_with_handle(Handle::TopLeft, (500.0, 500.0), (1000.0, 800.0), 1.0, false);
        assert_eq!(shrunk.width(), 32.0);
        assert_eq!(shrunk.height(), 24.0);
        assert_eq!(shrunk.right, 300.0);
        assert_eq!(shrunk.bottom, 220.0);

        let moved = rect.translate((900.0, -200.0), (1000.0, 800.0));
        assert_eq!(moved.width(), 200.0);
        assert_eq!(moved.height(), 120.0);
        assert_eq!(moved.right, 1000.0);
        assert_eq!(moved.top, 0.0);
    }

    #[test]
    fn floating_toolbar_flips_above_and_inside_near_screen_edges() {
        let bounds = (1280.0, 720.0);
        let below = place_floating_bars(
            Rect {
                left: 100.0,
                top: 100.0,
                right: 500.0,
                bottom: 300.0,
            },
            (320.0, 36.0),
            Some((200.0, 30.0)),
            bounds,
            1.0,
        );
        assert!(!below.inside_selection);
        assert!(below.bar.top > 300.0);
        assert!(below.subbar.unwrap().top > below.bar.bottom);

        let above = place_floating_bars(
            Rect {
                left: 100.0,
                top: 300.0,
                right: 500.0,
                bottom: 690.0,
            },
            (320.0, 36.0),
            Some((200.0, 30.0)),
            bounds,
            1.0,
        );
        assert!(!above.inside_selection);
        assert!(above.bar.bottom < 300.0);
        assert!(above.subbar.unwrap().bottom < above.bar.top);

        let fullscreen = place_floating_bars(
            Rect {
                left: 0.0,
                top: 0.0,
                right: 1280.0,
                bottom: 720.0,
            },
            (320.0, 36.0),
            Some((200.0, 30.0)),
            bounds,
            1.0,
        );
        assert!(fullscreen.inside_selection);
        assert!(fullscreen.bar.bottom <= 720.0 - 12.0);
        assert!(fullscreen.subbar.unwrap().bottom < fullscreen.bar.top);
    }
}
