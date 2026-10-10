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
    source: Option<ID3D11Texture2D>,
    target: ID3D11Texture2D,
    text_factory: IDWriteFactory,
    text: IDWriteTextFormat,
    status_text: IDWriteTextFormat,
    dim: ID2D1SolidColorBrush,
    line: ID2D1SolidColorBrush,
    caution: ID2D1SolidColorBrush,
    pill_fill: ID2D1SolidColorBrush,
    pill_border: ID2D1SolidColorBrush,
    hint: ID2D1SolidColorBrush,
    primary: ID2D1SolidColorBrush,
    scale: f32,
    white_scale: f32,
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
        white_scale: f32,
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
                    Format: DXGI_FORMAT_R16G16B16A16_FLOAT,
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
            swap.cast::<IDXGISwapChain3>()?
                .SetColorSpace1(DXGI_COLOR_SPACE_RGB_FULL_G10_NONE_P709)?;
            let target_texture = swap.GetBuffer::<ID3D11Texture2D>(0)?;
            let context = D2D1CreateDevice(&dxgi, None)?
                .CreateDeviceContext(D2D1_DEVICE_CONTEXT_OPTIONS_NONE)?;
            let target = context.CreateBitmapFromDxgiSurface(
                &swap.GetBuffer::<IDXGISurface>(0)?,
                Some(&D2D1_BITMAP_PROPERTIES1 {
                    pixelFormat: D2D1_PIXEL_FORMAT {
                        format: DXGI_FORMAT_R16G16B16A16_FLOAT,
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
            let text_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let text = text_format(&text_factory, 11.0 * scale, DWRITE_FONT_WEIGHT_NORMAL)?;
            let status_text = text_format(&text_factory, 12.0 * scale, DWRITE_FONT_WEIGHT_MEDIUM)?;
            let dim = context.CreateSolidColorBrush(&color(0.0, 0.0, 0.0, 0.55), None)?;
            let line =
                context.CreateSolidColorBrush(&token_color("accentBase", white_scale)?, None)?;
            let caution =
                context.CreateSolidColorBrush(&token_color("statusCaution", white_scale)?, None)?;
            let pill_fill =
                context.CreateSolidColorBrush(&token_color("surfaceWindow", white_scale)?, None)?;
            let pill_border =
                context.CreateSolidColorBrush(&token_color("borderDefault", white_scale)?, None)?;
            let hint =
                context.CreateSolidColorBrush(&token_color("textSecondary", white_scale)?, None)?;
            let primary =
                context.CreateSolidColorBrush(&token_color("textPrimary", white_scale)?, None)?;
            Ok(Self {
                swap,
                immediate: device.context.clone(),
                context,
                source: Some(surface.clone()),
                target: target_texture,
                text_factory,
                text,
                status_text,
                dim,
                line,
                caution,
                pill_fill,
                pill_border,
                hint,
                primary,
                scale,
                white_scale,
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
        self.render(selection, pointer, capturing)?;
        // SAFETY: The worker owns the completed swap-chain back buffer.
        unsafe { self.swap.Present(0, DXGI_PRESENT::default()).ok() }
    }
    fn render(&self, selection: Option<Rect>, pointer: (f64, f64), capturing: bool) -> Result<()> {
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
            // Same-size FP16 GPU copy preserves scRGB, including HDR and negative values.
            // Only the independent swap-chain target receives UI; the frozen source is read-only.
            self.immediate.CopyResource(&self.target, source);
            c.BeginDraw();
            if let Some(r) = selection {
                // Dim only outside the selection; the four rectangles never overlap.
                let fill = |bounds: D2D_RECT_F| {
                    if bounds.right > bounds.left && bounds.bottom > bounds.top {
                        c.FillRectangle(&bounds, &self.dim);
                    }
                };
                let (left, top, right, bottom) =
                    (r.left as f32, r.top as f32, r.right as f32, r.bottom as f32);
                fill(rect(0.0, 0.0, self.width, top));
                fill(rect(0.0, top, left, bottom));
                fill(rect(right, top, self.width, bottom));
                fill(rect(0.0, bottom, self.width, self.height));
            }
            if !capturing && (selection.is_none() || !valid) {
                c.DrawLine(
                    Vector2 {
                        X: pointer.0 as f32,
                        Y: 0.0,
                    },
                    Vector2 {
                        X: pointer.0 as f32,
                        Y: self.height,
                    },
                    &self.line,
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
                    &self.line,
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
            Ok(())
        }
    }
    pub fn attach(
        &mut self,
        surface: &ID3D11Texture2D,
        scale: f32,
        white_scale: f32,
    ) -> Result<()> {
        self.detach();
        if self.scale != scale {
            self.text = text_format(&self.text_factory, 11.0 * scale, DWRITE_FONT_WEIGHT_NORMAL)?;
            self.status_text =
                text_format(&self.text_factory, 12.0 * scale, DWRITE_FONT_WEIGHT_MEDIUM)?;
            self.scale = scale;
        }
        if self.white_scale != white_scale {
            for (brush, token) in [
                (&self.line, "accentBase"),
                (&self.caution, "statusCaution"),
                (&self.pill_fill, "surfaceWindow"),
                (&self.pill_border, "borderDefault"),
                (&self.hint, "textSecondary"),
                (&self.primary, "textPrimary"),
            ] {
                let value = token_color(token, white_scale)?;
                // SAFETY: Brushes are owned and updated exclusively on this worker.
                unsafe { brush.SetColor(&value) };
            }
            self.white_scale = white_scale;
        }
        self.source = Some(surface.clone());
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
        // SAFETY: Worker-exclusive context use; release bindings before dropping its frozen texture.
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
fn token_color(name: &str, white_scale: f32) -> Result<D2D1_COLOR_F> {
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
        srgb_to_linear(((value >> 16) & 255) as f32 / 255.0) * white_scale,
        srgb_to_linear(((value >> 8) & 255) as f32 / 255.0) * white_scale,
        srgb_to_linear((value & 255) as f32 / 255.0) * white_scale,
        1.0,
    ))
}
fn srgb_to_linear(value: f32) -> f32 {
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        capture::FrozenFrame,
        graphics::{display_color, rgba16f_to_rgba8},
        interop::{Apartment, DpiScope, cursor_monitor},
        overlay::Overlay,
    };
    use half::f16;
    use windows::Win32::System::WinRT::RO_INIT_MULTITHREADED;

    #[test]
    #[ignore = "Real D3D/Direct2D FP16 presentation/readback; run explicitly"]
    fn fp16_preview_preserves_background_and_png_bytes() {
        let _apartment = Apartment::initialize(RO_INIT_MULTITHREADED).unwrap();
        let _dpi = DpiScope::physical_coordinates().unwrap();
        let device = Device::create().unwrap();
        let monitor = cursor_monitor().unwrap();
        let display = display_color(&monitor).unwrap();
        println!(
            "GPU fixture target: {}, range={:?}, outputScale={}",
            monitor.name, display.range, display.scale
        );
        let texture = device.texture(128, 96, false).unwrap();
        let pixel: Vec<u8> = [1.5, 0.25, -0.25, 1.0]
            .into_iter()
            .flat_map(|v| f16::from_f32(v).to_le_bytes())
            .collect();
        let source = pixel.repeat(128 * 96);
        // SAFETY: Owned same-size texture, initialized bytes and tightly packed FP16 rows.
        unsafe {
            device
                .context
                .UpdateSubresource(&texture, 0, None, source.as_ptr().cast(), 128 * 8, 0)
        };
        let frame = FrozenFrame {
            texture,
            width: 128,
            height: 96,
            color: display,
            monitor,
        };
        let overlay = Overlay::new(&frame).unwrap();
        let mut presenter =
            Presenter::new(&device, overlay.window.0, &frame.texture, 128, 96, 1.0, 1.0).unwrap();
        let expected_pixel = |bytes: &[u8], x: usize, y: usize| {
            bytes[(y * 128 + x) * 8..(y * 128 + x + 1) * 8].to_vec()
        };
        for output_scale in [1.0, 0.25] {
            let baseline =
                crate::delivery::encode_png(128, 96, &rgba16f_to_rgba8(&source, 128, output_scale))
                    .unwrap();
            presenter
                .attach(&frame.texture, 1.0, output_scale.recip())
                .unwrap();
            for selection in [
                None,
                Some(Rect {
                    left: 16.0,
                    top: 16.0,
                    right: 80.0,
                    bottom: 56.0,
                }),
                None,
            ] {
                // Read before Present rotates the flip-model back buffers.
                presenter.render(selection, (-1.0, -1.0), false).unwrap();
                let rendered = device.readback(&presenter.target, 128, 96, None).unwrap();
                assert_eq!(
                    expected_pixel(&rendered, 40, 40),
                    pixel,
                    "Unmasked scRGB pixels must retain HDR and negative values"
                );
                if selection.is_some() {
                    let outside = expected_pixel(&rendered, 100, 20);
                    let red = f16::from_le_bytes([outside[0], outside[1]]).to_f32();
                    assert!(
                        (red - 1.5 * 0.45).abs() < 0.002,
                        "Mask must dim only the independent target"
                    );
                } else {
                    assert_eq!(
                        expected_pixel(&rendered, 100, 20),
                        pixel,
                        "No residual mask on the next draw"
                    );
                }
                let retained = device.readback(&frame.texture, 128, 96, None).unwrap();
                assert_eq!(retained, source);
                assert_eq!(
                    crate::delivery::encode_png(
                        128,
                        96,
                        &rgba16f_to_rgba8(&retained, 128, output_scale)
                    )
                    .unwrap(),
                    baseline
                );
            }
        }
    }
}
