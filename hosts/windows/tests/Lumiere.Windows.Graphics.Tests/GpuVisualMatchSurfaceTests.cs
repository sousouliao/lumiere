using System.Runtime.InteropServices;
using Lumiere.Windows.Graphics.Devices;
using Lumiere.Windows.Graphics.Hdr;
using Lumiere.Windows.Graphics.Output;
using Lumiere.Windows.Graphics.Presentation;
using Vortice.Direct3D;
using Vortice.Direct3D11;
using Vortice.DXGI;
using Vortice.Mathematics;
using Xunit;

namespace Lumiere.Windows.Graphics.Tests;

public sealed class GpuVisualMatchSurfaceTests
{
    [Theory]
    [InlineData(80f)]
    [InlineData(160f)]
    public void FullPixelGpuSurfaceMatchesOutputConversionWithinQuantization(float sdrWhiteNits)
    {
        var d3d = D3D11.D3D11CreateDevice(
            DriverType.Warp,
            DeviceCreationFlags.BgraSupport,
            FeatureLevel.Level_11_0);
        using var resources = new GraphicsDeviceResources(
            d3d,
            d3d.ImmediateContext,
            d3d.QueryInterface<IDXGIDevice>(),
            "WARP",
            d3d.FeatureLevel,
            EngineReadinessStatus.Ready("WARP ready", "GPU conversion fixture"));
        byte[] linear = new byte[8];
        WriteHalf(linear, 0, 0.5f);
        WriteHalf(linear, 2, 1f);
        WriteHalf(linear, 4, 2f);
        WriteHalf(linear, 6, 1f);
        var data = Marshal.AllocHGlobal(linear.Length);
        try
        {
            Marshal.Copy(linear, 0, data, linear.Length);
            var description = new Texture2DDescription
            {
                Width = 1,
                Height = 1,
                MipLevels = 1,
                ArraySize = 1,
                Format = Format.R16G16B16A16_Float,
                SampleDescription = new SampleDescription(1, 0),
                Usage = ResourceUsage.Default,
                BindFlags = BindFlags.ShaderResource,
            };
            using var source = new CapturedFrameTexture(
                d3d.CreateTexture2D(description, new SubresourceData(data, 8)),
                1,
                1,
                "GPU fixture");
            using var converter = new GpuVisualMatchSurface(resources);
            var context = SrgbVisualMatchConversionContext.ForHdrDisplay(sdrWhiteNits);
            using var surface = converter.Render(source, context);
            using var staging = d3d.CreateTexture2D(new Texture2DDescription
            {
                Width = 1,
                Height = 1,
                MipLevels = 1,
                ArraySize = 1,
                Format = Format.B8G8R8A8_UNorm,
                SampleDescription = new SampleDescription(1, 0),
                Usage = ResourceUsage.Staging,
                BindFlags = BindFlags.None,
                CPUAccessFlags = CpuAccessFlags.Read,
            });
            resources.ImmediateContext.CopyResource(staging, surface.Texture!);
            var mapped = resources.ImmediateContext.Map(staging, 0, MapMode.Read, Vortice.Direct3D11.MapFlags.None);
            byte[] actual = new byte[4];
            try
            {
                Marshal.Copy(mapped.DataPointer, actual, 0, actual.Length);
            }
            finally
            {
                resources.ImmediateContext.Unmap(staging, 0);
            }

            var expected = SrgbVisualMatchPixelConverter.ConvertRgba16FloatToBgra8(
                new CapturedFrameReadback(1, 1, linear), context).Bgra8PixelData;
            for (var channel = 0; channel < 4; channel++)
            {
                Assert.InRange(Math.Abs(actual[channel] - expected[channel]), 0, 2);
            }
        }
        finally
        {
            Marshal.FreeHGlobal(data);
        }
    }

    private static void WriteHalf(byte[] bytes, int offset, float value)
    {
        var bits = BitConverter.HalfToUInt16Bits((Half)value);
        bytes[offset] = (byte)bits;
        bytes[offset + 1] = (byte)(bits >> 8);
    }
}
