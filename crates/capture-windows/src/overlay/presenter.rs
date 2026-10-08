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
    text: IDWriteTextFormat,
    dim: ID2D1SolidColorBrush,
    line: ID2D1SolidColorBrush,
    hint: ID2D1SolidColorBrush,
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
            let text = text_factory.CreateTextFormat(
                w!("Segoe UI"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                15.0,
                w!("en-us"),
            )?;
            let dim = context.CreateSolidColorBrush(&color(0.0, 0.0, 0.0, 0.42), None)?;
            let line = context.CreateSolidColorBrush(&color(0.1, 0.65, 1.0, 1.0), None)?;
            let hint = context.CreateSolidColorBrush(&color(1.0, 1.0, 1.0, 1.0), None)?;
            Ok(Self {
                swap,
                immediate: device.context.clone(),
                context,
                source: Some(source),
                text,
                dim,
                line,
                hint,
                width: width as f32,
                height: height as f32,
            })
        }
    }
    pub fn draw(&self, selection: Option<Rect>, pointer: (f64, f64)) -> Result<()> {
        let source = self.source.as_ref().ok_or_else(|| {
            windows::core::Error::new(
                windows::Win32::Foundation::E_FAIL,
                "No frozen Region surface",
            )
        })?;
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
                c.DrawRectangle(&rect(left, top, right, bottom), &self.line, 2.0, None);
            } else {
                fill(rect(0.0, 0.0, self.width, self.height));
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
                    1.0,
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
                    1.0,
                    None,
                );
            }
            let hint: Vec<u16> = "Drag to select  ·  Esc or right-click to cancel"
                .encode_utf16()
                .collect();
            c.DrawText(
                &hint,
                &self.text,
                &rect(20.0, 18.0, 580.0, 64.0),
                &self.hint,
                D2D1_DRAW_TEXT_OPTIONS_NONE,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            c.EndDraw(None, None)?;
            self.swap.Present(0, DXGI_PRESENT::default()).ok()?;
            Ok(())
        }
    }
    pub fn attach(&mut self, surface: &ID3D11Texture2D) -> Result<()> {
        self.detach();
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
    pub fn detach(&mut self) {
        // SAFETY: Worker-exclusive context use; release bindings before dropping its frozen bitmap.
        unsafe {
            self.immediate.ClearState();
            self.immediate.Flush();
        }
        self.source = None;
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
