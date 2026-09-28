using System.Runtime.InteropServices;
using Lumiere.Windows.Graphics.Devices;
using Lumiere.Windows.Graphics.Hdr;
using Lumiere.Windows.Graphics.Presentation;
using Vortice.Direct3D;
using Vortice.Direct3D11;
using Vortice.DXGI;
using Vortice.Mathematics;

namespace Lumiere.Windows.Graphics.Output;

internal interface ICapturedFrameTextureReadback
{
    CapturedFrameReadback ReadRgba16Float(
        CapturedFrameTexture texture,
        CropPixelRect? cropRegion);
}

internal sealed record CapturedFrameReadback
{
    public CapturedFrameReadback(
        int width,
        int height,
        byte[] pixelData)
    {
        if (width <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(Width), Width, "Readback width must be positive.");
        }

        if (height <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(Height), Height, "Readback height must be positive.");
        }

        ArgumentNullException.ThrowIfNull(pixelData);
        var expectedLength = checked(width * height * BytesPerPixel);
        if (pixelData.Length != expectedLength)
        {
            throw new ArgumentException(
                $"Readback pixel data length must be {expectedLength} bytes for {width}x{height} R16G16B16A16 float data.",
                nameof(pixelData));
        }

        Width = width;
        Height = height;
        PixelData = pixelData;
    }

    public const int BytesPerPixel = 8;

    public int Width { get; }

    public int Height { get; }

    public byte[] PixelData { get; }
}

internal sealed class CapturedFrameTextureReadback : ICapturedFrameTextureReadback
{
    private readonly GraphicsDeviceResources deviceResources;

    public CapturedFrameTextureReadback(GraphicsDeviceResources deviceResources)
    {
        this.deviceResources = deviceResources ?? throw new ArgumentNullException(nameof(deviceResources));
    }

    public CapturedFrameReadback ReadRgba16Float(
        CapturedFrameTexture texture,
        CropPixelRect? cropRegion)
    {
        ArgumentNullException.ThrowIfNull(texture);
        if (texture.Texture is null)
        {
            throw new OutputArtifactEncodingException("Captured frame texture is unavailable for HDR readback.");
        }

        int pixelX = cropRegion?.X ?? 0;
        int pixelY = cropRegion?.Y ?? 0;
        int pixelWidth = cropRegion?.Width ?? texture.Width;
        int pixelHeight = cropRegion?.Height ?? texture.Height;

        if (!ValidateRegion(pixelX, pixelY, pixelWidth, pixelHeight, texture.Width, texture.Height))
        {
            throw new OutputArtifactEncodingException("Invalid HDR readback crop region.");
        }

        using var stagingTexture = CreateStagingTexture(pixelWidth, pixelHeight);
        var sourceBox = new Box(pixelX, pixelY, 0, pixelX + pixelWidth, pixelY + pixelHeight, 1);
        lock (deviceResources.ImmediateContextSync)
        {
            deviceResources.ImmediateContext.CopySubresourceRegion(
                stagingTexture,
                0,
                0,
                0,
                0,
                texture.Texture,
                0,
                sourceBox);

            var map = deviceResources.ImmediateContext.Map(
                stagingTexture,
                0,
                MapMode.Read,
                Vortice.Direct3D11.MapFlags.None);
            try
            {
                var pixelData = new byte[checked(pixelWidth * pixelHeight * CapturedFrameReadback.BytesPerPixel)];
                var rowBytes = checked(pixelWidth * CapturedFrameReadback.BytesPerPixel);
                var stride = checked((int)map.RowPitch);

                for (int y = 0; y < pixelHeight; y++)
                {
                    Marshal.Copy(
                        IntPtr.Add(map.DataPointer, y * stride),
                        pixelData,
                        y * rowBytes,
                        rowBytes);
                }

                return new CapturedFrameReadback(
                    pixelWidth,
                    pixelHeight,
                    pixelData);
            }
            finally
            {
                deviceResources.ImmediateContext.Unmap(stagingTexture, 0);
            }
        }
    }

    private ID3D11Texture2D CreateStagingTexture(int width, int height)
    {
        var description = new Texture2DDescription
        {
            Width = (uint)width,
            Height = (uint)height,
            MipLevels = 1,
            ArraySize = 1,
            Format = HdrConstants.DxgiTextureFormat,
            SampleDescription = new SampleDescription(1, 0),
            Usage = ResourceUsage.Staging,
            BindFlags = BindFlags.None,
            CPUAccessFlags = CpuAccessFlags.Read,
            MiscFlags = ResourceOptionFlags.None,
        };

        return deviceResources.Device.CreateTexture2D(description);
    }

    private static bool ValidateRegion(
        int x,
        int y,
        int width,
        int height,
        int sourceWidth,
        int sourceHeight)
    {
        return x >= 0
            && y >= 0
            && width > 0
            && height > 0
            && x + width <= sourceWidth
            && y + height <= sourceHeight;
    }
}
