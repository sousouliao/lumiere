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
impl Rect {
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
#[cfg(test)]
mod tests {
    use super::*;
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
}
