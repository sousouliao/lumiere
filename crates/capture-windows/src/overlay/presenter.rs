use super::geometry::Rect;
use crate::graphics::Device;
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct2D::{Common::*, *},
            Direct3D11::ID3D11Texture2D,
            DirectWrite::*,
            Dxgi::{Common::*, *},
        },
    },
    core::{Interface, Result, w},
};
use windows_numerics::Vector2;

pub(super) struct Presenter {
    swap: IDXGISwapChain1,
    immediate: windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
    context: ID2D1DeviceContext,
    source: Option<ID2D1Bitmap1>,
    text_factory: IDWriteFactory,
    text: IDWriteTextFormat,
    status_text: IDWriteTextFormat,
    dim: ID2D1SolidColorBrush,
    line: ID2D1SolidColorBrush,
    crosshair: ID2D1SolidColorBrush,
    caution: ID2D1SolidColorBrush,
    pill_fill: ID2D1SolidColorBrush,
    pill_border: ID2D1SolidColorBrush,
    hint: ID2D1SolidColorBrush,
    primary: ID2D1SolidColorBrush,
    scale: f32,
    width: f32,
    height: f32,
}
impl Presenter {
    pub fn new(
        device: &Device,
        window: HWND,
        surface: &ID3D11Texture2D,
        width: u32,
        height: u32,
        scale: f32,
    ) -> Result<Self> {
        // SAFETY: The current worker owns the window, device/context, textures and all created COM resources.
        unsafe {
            let dxgi: IDXGIDevice = device.native.cast()?;
            let factory: IDXGIFactory2 = dxgi.GetAdapter()?.GetParent()?;
            let swap = factory.CreateSwapChainForHwnd(
                &device.native,
                window,
                &DXGI_SWAP_CHAIN_DESC1 {
                    Width: width,
                    Height: height,
                    Format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    SampleDesc: DXGI_SAMPLE_DESC {
                        Count: 1,
                        Quality: 0,
                    },
                    BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
                    BufferCount: 2,
                    Scaling: DXGI_SCALING_STRETCH,
                    SwapEffect: DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL,
                    AlphaMode: DXGI_ALPHA_MODE_IGNORE,
                    ..Default::default()
                },
                None,
                None,
            )?;
            let context = D2D1CreateDevice(&dxgi, None)?
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let target = context.CreateBitmapFromDxgiSurface(
                &swap.GetBuffer::<IDXGISurface>(0)?,
                Some(&D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_IGNORE,
                    },
                    dpiX: 96.0,
                    dpiY: 96.0,
                    bitmapOptions: D2D1_BITMAP_OPTIONS_TARGET | D2D1_BITMAP_OPTIONS_CANNOT_DRAW,
                    ..Default::default()
                }),
            )?;
            context.SetTarget(&target);
            context.SetDpi(96.0, 96.0);
            let source = context.CreateBitmapFromDxgiSurface(
                &surface.cast::<IDXGISurface>()?,
                Some(&D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_IGNORE,
                    },
                    dpiX: 96.0,
                    dpiY: 96.0,
                    ..Default::default()
                }),
            )?;
            let text_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let text = text_format(&text_factory, 11.0 * scale, DWRITE_FONT_WEIGHT_NORMAL)?;
            let status_text = text_format(&text_factory, 12.0 * scale, DWRITE_FONT_WEIGHT_MEDIUM)?;
            let dim = context.CreateSolidColorBrush(&color(0.0, 0.0, 0.0, 0.55), None)?;
            let line = context.CreateSolidColorBrush(&token_color("accentBase")?, None)?;
            let crosshair = context.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 0.16), None)?;
            let caution = context.CreateSolidColorBrush(&token_color("statusCaution")?, None)?;
            let pill_fill = context.CreateSolidColorBrush(&token_color("surfaceWindow")?, None)?;
            let pill_border =
                context.CreateSolidColorBrush(&token_color("borderDefault")?, None)?;
            let hint = context.CreateSolidColorBrush(&token_color("textSecondary")?, None)?;
            let primary = context.CreateSolidColorBrush(&token_color("textPrimary")?, None)?;
            Ok(Self {
                swap,
                immediate: device.context.clone(),
                context,
                source: Some(source),
                text_factory,
                text,
                status_text,
                dim,
                line,
                crosshair,
                caution,
                pill_fill,
                pill_border,
                hint,
                primary,
                scale,
                width: width as f32,
                height: height as f32,
            })
        }
    }
    pub fn draw(
        &self,
        selection: Option<Rect>,
        pointer: (f64, f64),
        capturing: bool,
    ) -> Result<()> {
        let source = self.source.as_ref().ok_or_else(|| {
            windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "No frozen Region surface",
            )
        })?;
        let scale = self.scale;
        let valid = selection.is_some_and(|r| r.is_valid_selection(scale as f64));
        let hint = (!capturing)
            .then(|| {
                self.pill(
                    if selection.is_none() {
                        "Drag to select · Esc cancels"
                    } else if valid {
                        "Release to capture · Esc cancels"
                    } else {
                        "Keep dragging · Esc cancels"
                    },
                    &self.text,
                    (12.0, 7.0),
                    7.0,
                    false,
                )
            })
            .transpose()?;
        let readout = selection
            .map(|r| {
                let crop = r.crop(
                    (self.width as f64, self.height as f64),
                    (self.width as u32, self.height as u32),
                );
                let label = if capturing {
                    "Capturing…".to_owned()
                } else if let Some(crop) = crop {
                    format!(
                        "{} × {}{}",
                        crop.width,
                        crop.height,
                        if valid { "" } else { " · too small" }
                    )
                } else {
                    "Too small".to_owned()
                };
                self.pill(
                    &label,
                    if capturing {
                        &self.status_text
                    } else {
                        &self.text
                    },
                    if capturing { (10.0, 7.0) } else { (8.0, 5.0) },
                    if capturing { 7.0 } else { 6.0 },
                    capturing,
                )
            })
            .transpose()?;
        // SAFETY: All drawing resources belong to this worker; geometry is bounded to the target.
        unsafe {
            let c = &self.context;
            c.BeginDraw();
            c.Clear(Some(&color(0.0, 0.0, 0.0, 1.0)));
            c.DrawBitmap(
                source,
                Some(&rect(0.0, 0.0, self.width, self.height)),
                1.0,
                D2D1_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                None,
                None,
            );
            let fill = |r: D2D_RECT_F| {
                if r.right > r.left && r.bottom > r.top {
                    c.FillRectangle(&r, &self.dim);
                }
            };
            if let Some(r) = selection {
                let (left, top, right, bottom) =
                    (r.left as f32, r.top as f32, r.right as f32, r.bottom as f32);
                fill(rect(0.0, 0.0, self.width, top));
                fill(rect(0.0, top, left, bottom));
                fill(rect(right, top, self.width, bottom));
                fill(rect(0.0, bottom, self.width, self.height));
            } else {
                fill(rect(0.0, 0.0, self.width, self.height));
            }
            if selection.is_none() || (!valid && !capturing) {
                c.DrawLine(
                    Vector2 {
                        X: pointer.0 as f32,
                        Y: 0.0,
                    },
                    Vector2 {
                        X: pointer.0 as f32,
                        Y: self.height,
                    },
                    &self.crosshair,
                    scale,
                    None,
                );
                c.DrawLine(
                    Vector2 {
                        X: 0.0,
                        Y: pointer.1 as f32,
                    },
                    Vector2 {
                        X: self.width,
                        Y: pointer.1 as f32,
                    },
                    &self.crosshair,
                    scale,
                    None,
                );
            }
            if let Some(r) = selection {
                // Keep the 1 DIP stroke inside the crop, including at display edges.
                let inset = scale.min((r.right - r.left).min(r.bottom - r.top) as f32) / 2.0;
                c.DrawRectangle(
                    &rect(
                        r.left as f32 + inset,
                        r.top as f32 + inset,
                        r.right as f32 - inset,
                        r.bottom as f32 - inset,
                    ),
                    if valid { &self.line } else { &self.caution },
                    inset * 2.0,
                    None,
                );
                if let Some(pill) = &readout {
                    let x = (r.left as f32).clamp(0.0, (self.width - pill.width).max(0.0));
                    let below = r.bottom as f32 + 8.0 * scale;
                    let bottom_limit = self.height
                        - 16.0 * scale
                        - hint.as_ref().map_or(0.0, |hint| hint.height + 8.0 * scale);
                    let y = if below + pill.height <= bottom_limit {
                        below
                    } else {
                        (r.top as f32 - 8.0 * scale - pill.height).max(0.0)
                    };
                    self.draw_pill(
                        pill,
                        (x, y),
                        if valid { &self.primary } else { &self.caution },
                        capturing,
                    );
                }
            }
            if let Some(hint) = &hint {
                self.draw_pill(
                    hint,
                    (
                        ((self.width - hint.width) / 2.0).max(0.0),
                        (self.height - 16.0 * scale - hint.height).max(0.0),
                    ),
                    &self.hint,
                    false,
                );
            }
            c.EndDraw(None, None)?;
            self.swap.Present(0, DXGI_PRESENT::default()).ok()?;
            Ok(())
        }
    }
    pub fn attach(&mut self, surface: &ID3D11Texture2D, scale: f32) -> Result<()> {
        self.detach();
        if self.scale != scale {
            self.text = text_format(&self.text_factory, 11.0 * scale, DWRITE_FONT_WEIGHT_NORMAL)?;
            self.status_text =
                text_format(&self.text_factory, 12.0 * scale, DWRITE_FONT_WEIGHT_MEDIUM)?;
            self.scale = scale;
        }
        // SAFETY: Texture belongs to this device/worker; the bitmap owns its source surface reference.
        self.source = Some(unsafe {
            self.context.CreateBitmapFromDxgiSurface(
                &surface.cast::<IDXGISurface>()?,
                Some(&D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_B8G8R8A8_UNORM,
                        alphaMode: D2D1_ALPHA_MODE_IGNORE,
                    },
                    dpiX: 96.0,
                    dpiY: 96.0,
                    ..Default::default()
                }),
            )?
        });
        Ok(())
    }
    fn pill(
        &self,
        label: &str,
        format: &IDWriteTextFormat,
        padding: (f32, f32),
        radius: f32,
        dot: bool,
    ) -> Result<Pill> {
        let label: Vec<u16> = label.encode_utf16().collect();
        let (padding_x, padding_y) = (padding.0 * self.scale, padding.1 * self.scale);
        // SAFETY: Text/layout resources are worker-owned; input lives through layout creation.
        unsafe {
            let layout =
                self.text_factory
                    .CreateTextLayout(&label, format, self.width, self.height)?;
            let mut metrics = DWRITE_TEXT_METRICS::default();
            layout.GetMetrics(&mut metrics)?;
            Ok(Pill {
                width: metrics.widthIncludingTrailingWhitespace.ceil()
                    + padding_x * 2.0
                    + if dot { 17.0 * self.scale } else { 0.0 },
                height: metrics.height.ceil() + padding_y * 2.0,
                layout,
                padding: (padding_x, padding_y),
                radius: radius * self.scale,
            })
        }
    }
    fn draw_pill(
        &self,
        pill: &Pill,
        position: (f32, f32),
        brush: &ID2D1SolidColorBrush,
        dot: bool,
    ) {
        // SAFETY: Called only inside the worker's BeginDraw/EndDraw pair with owned resources.
        unsafe {
            let (x, y) = position;
            let rounded = D2D1_ROUNDED_RECT {
                rect: rect(x, y, x + pill.width, y + pill.height),
                radiusX: pill.radius,
                radiusY: pill.radius,
            };
            self.context.FillRoundedRectangle(&rounded, &self.pill_fill);
            self.context
                .DrawRoundedRectangle(&rounded, &self.pill_border, self.scale, None);
            if dot {
                self.context.FillEllipse(
                    &D2D1_ELLIPSE {
                        point: Vector2 {
                            X: x + pill.padding.0 + 3.5 * self.scale,
                            Y: y + pill.height / 2.0,
                        },
                        radiusX: 3.5 * self.scale,
                        radiusY: 3.5 * self.scale,
                    },
                    &self.line,
                );
            }
            self.context.DrawTextLayout(
                Vector2 {
                    X: x + pill.padding.0 + if dot { 15.0 * self.scale } else { 0.0 },
                    Y: y + pill.padding.1,
                },
                &pill.layout,
                brush,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
            );
        }
    }
    pub fn detach(&mut self) {
        // SAFETY: Worker-exclusive context use; release bindings before dropping its frozen bitmap.
        unsafe {
            self.immediate.ClearState();
            self.immediate.Flush();
        }
        self.source = None;
    }
}
struct Pill {
    layout: IDWriteTextLayout,
    width: f32,
    height: f32,
    padding: (f32, f32),
    radius: f32,
}
fn text_format(
    factory: &IDWriteFactory,
    size: f32,
    weight: DWRITE_FONT_WEIGHT,
) -> Result<IDWriteTextFormat> {
    // SAFETY: The factory and returned format remain owned by the current worker.
    unsafe {
        let format = factory.CreateTextFormat(
            w!("Inter"),
            None,
            weight,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size,
            w!("en-us"),
        )?;
        format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        Ok(format)
    }
}
fn token_color(name: &str) -> Result<D2D1_COLOR_F> {
    // Reuse the Ardot export rather than maintaining a separate native palette.
    let tokens = include_str!("../../../../apps/desktop/src/renderer/src/tokens.generated.css");
    let prefix = format!("--Lumiere-Dark-{name}: #");
    let value = tokens
        .lines()
        .find_map(|line| line.trim().strip_prefix(&prefix))
        .and_then(|value| value.strip_suffix(';'))
        .and_then(|value| u32::from_str_radix(value, 16).ok())
        .ok_or_else(|| {
            windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "Missing Region design color",
            )
        })?;
    Ok(color(
        ((value >> 16) & 255) as f32 / 255.0,
        ((value >> 8) & 255) as f32 / 255.0,
        (value & 255) as f32 / 255.0,
        1.0,
    ))
}
impl Drop for Presenter {
    fn drop(&mut self) {
        // SAFETY: Release context's target reference before its owned swap chain/resources.
        unsafe {
            self.context.SetTarget(None);
            self.immediate.ClearState();
            self.immediate.Flush();
        }
    }
}
fn color(r: f32, g: f32, b: f32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F { r, g, b, a }
}
fn rect(left: f32, top: f32, right: f32, bottom: f32) -> D2D_RECT_F {
    D2D_RECT_F {
        left,
        top,
        right,
        bottom,
    }
}
