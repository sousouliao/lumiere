using System.Numerics;
using Lumiere.Windows.Graphics.Devices;
using Vortice;
using Vortice.DCommon;
using Vortice.Direct2D1;
using Vortice.DirectWrite;
using Vortice.DXGI;
using Vortice.Mathematics;

namespace Lumiere.Windows.Graphics.Presentation;

/// <summary>
/// Draws the GPU Visual Match texture and Region affordances into a window swap chain.
/// The owner calls this only on the overlay UI thread; the shared D3D11 context is
/// serialized with WGC and conversion through GraphicsDeviceResources.ImmediateContextSync.
/// </summary>
internal sealed class NativeRegionPresenter : IDisposable
{
    private readonly GraphicsDeviceResources device;
    private readonly IDXGISwapChain1 swapChain;
    private readonly ID2D1Device d2dDevice;
    private readonly ID2D1DeviceContext context;
    private readonly ID2D1Bitmap1 targetBitmap;
    private readonly IDWriteFactory textFactory;
    private readonly IDWriteTextFormat textFormat;
    private readonly ID2D1SolidColorBrush dimBrush;
    private readonly ID2D1SolidColorBrush lineBrush;
    private readonly ID2D1SolidColorBrush hintBrush;
    private ID2D1Bitmap1? sourceBitmap;
    private readonly int width;
    private readonly int height;
    private bool disposed;

    public NativeRegionPresenter(GraphicsDeviceResources device, nint window, int width, int height)
    {
        this.device = device ?? throw new ArgumentNullException(nameof(device));
        if (window == 0 || width <= 0 || height <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(window));
        }

        this.width = width;
        this.height = height;
        try
        {
            device.DxgiDevice.GetAdapter(out var adapter);
            using var ownedAdapter = adapter;
            using var factory = adapter.GetParent<IDXGIFactory2>();
            swapChain = factory.CreateSwapChainForHwnd(
                device.Device,
                window,
                new SwapChainDescription1(
                    (uint)width,
                    (uint)height,
                    Format.B8G8R8A8_UNorm,
                    false,
                    Usage.RenderTargetOutput,
                    2,
                    Scaling.Stretch,
                    SwapEffect.FlipSequential,
                    Vortice.DXGI.AlphaMode.Ignore,
                    SwapChainFlags.None));
            d2dDevice = D2D1.D2D1CreateDevice(device.DxgiDevice);
            context = d2dDevice.CreateDeviceContext(DeviceContextOptions.None);
            using var backBuffer = swapChain.GetBuffer<IDXGISurface>(0);
            targetBitmap = context.CreateBitmapFromDxgiSurface(
                backBuffer,
                new BitmapProperties1(
                    new PixelFormat(Format.B8G8R8A8_UNorm, Vortice.DCommon.AlphaMode.Ignore),
                    96,
                    96,
                    BitmapOptions.Target | BitmapOptions.CannotDraw));
            context.Target = targetBitmap;
            textFactory = DWrite.DWriteCreateFactory<IDWriteFactory>();
            textFormat = textFactory.CreateTextFormat(
                "Segoe UI",
                null,
                FontWeight.Normal,
                FontStyle.Normal,
                FontStretch.Normal,
                15,
                "en-us");
            dimBrush = context.CreateSolidColorBrush(new Color4(0, 0, 0, 0.42f));
            lineBrush = context.CreateSolidColorBrush(new Color4(0.1f, 0.65f, 1, 1));
            hintBrush = context.CreateSolidColorBrush(new Color4(1, 1, 1, 1));
        }
        catch
        {
            Dispose();
            throw;
        }
    }

    public void Attach(CapturedFrameTexture visualMatchSurface)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        ArgumentNullException.ThrowIfNull(visualMatchSurface);
        if (visualMatchSurface.Texture is null
            || visualMatchSurface.Width != width
            || visualMatchSurface.Height != height)
        {
            throw new InvalidOperationException("The overlay surface does not match its target window.");
        }

        Detach();
        using var sourceSurface = visualMatchSurface.Texture.QueryInterface<IDXGISurface>();
        sourceBitmap = context.CreateBitmapFromDxgiSurface(
            sourceSurface,
            new BitmapProperties1(
                new PixelFormat(Format.B8G8R8A8_UNorm, Vortice.DCommon.AlphaMode.Ignore)));
    }

    public void Draw(NativeRegionDrawing drawing)
    {
        ObjectDisposedException.ThrowIf(disposed, this);
        var source = sourceBitmap
            ?? throw new InvalidOperationException("The overlay has no matching frozen surface.");

        lock (device.ImmediateContextSync)
        {
            context.BeginDraw();
            context.Clear(new Color4(0, 0, 0, 1));
            context.DrawBitmap(source, new RawRectF(0, 0, width, height),
                1, InterpolationMode.NearestNeighbor, null, null);

            var selection = drawing.Selection;
            if (selection is { } rect)
            {
                FillDim(0, 0, width, rect.Top);
                FillDim(0, rect.Top, rect.Left, rect.Bottom);
                FillDim(rect.Right, rect.Top, width, rect.Bottom);
                FillDim(0, rect.Bottom, width, height);
                var outline = new RawRectF(rect.Left, rect.Top, rect.Right, rect.Bottom);
                context.DrawRectangle(outline, lineBrush, 2);
            }
            else
            {
                FillDim(0, 0, width, height);
                context.DrawLine(new Vector2(drawing.PointerX, 0),
                    new Vector2(drawing.PointerX, height), lineBrush, 1);
                context.DrawLine(new Vector2(0, drawing.PointerY),
                    new Vector2(width, drawing.PointerY), lineBrush, 1);
            }

            context.DrawText("Drag to select  ·  Esc or right-click to cancel",
                textFormat, new Rect(20, 18, 560, 46), hintBrush);
            context.EndDraw().CheckError();
            swapChain.Present(0, PresentFlags.None).CheckError();
        }
    }

    public void Detach()
    {
        sourceBitmap?.Dispose();
        sourceBitmap = null;
    }

    private void FillDim(float left, float top, float right, float bottom)
    {
        if (right > left && bottom > top)
        {
            context.FillRectangle(new RawRectF(left, top, right, bottom), dimBrush);
        }
    }

    public void Dispose()
    {
        if (disposed) return;
        disposed = true;
        Detach();
        hintBrush?.Dispose();
        lineBrush?.Dispose();
        dimBrush?.Dispose();
        textFormat?.Dispose();
        textFactory?.Dispose();
        if (context is not null) context.Target = null!;
        targetBitmap?.Dispose();
        context?.Dispose();
        d2dDevice?.Dispose();
        swapChain?.Dispose();
    }
}

internal readonly record struct NativeRegionDrawing(
    NativeRegionPixelRect? Selection,
    float PointerX,
    float PointerY);

internal readonly record struct NativeRegionPixelRect(
    float Left,
    float Top,
    float Right,
    float Bottom);
