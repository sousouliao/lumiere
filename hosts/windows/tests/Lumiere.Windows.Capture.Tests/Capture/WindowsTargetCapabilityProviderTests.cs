using Lumiere.Windows.Graphics.Hdr;
using Lumiere.Windows.Interop;
using Xunit;

namespace Lumiere.Windows.Capture.Tests;

public sealed class WindowsTargetCapabilityProviderTests
{
    [Theory]
    [InlineData(HdrDisplayState.Active, WindowsTargetHdrState.Active)]
    [InlineData(HdrDisplayState.Inactive, WindowsTargetHdrState.Inactive)]
    [InlineData(HdrDisplayState.Unknown, WindowsTargetHdrState.Unknown)]
    public void ReportsSelectedTargetLogicalSizeAndHdrState(
        HdrDisplayState hdrState,
        WindowsTargetHdrState expectedState)
    {
        var monitor = new MonitorHandle(
            (nint)42,
            @"\\.\DISPLAY2",
            Width: 3840,
            Height: 2160,
            EffectiveDpiX: 144,
            EffectiveDpiY: 144);
        var provider = new WindowsTargetCapabilityProvider(
            () => monitor,
            _ => new HdrDisplayCapability(hdrState, null, @"\\.\DISPLAY2"));

        var capability = provider.GetCurrent();

        Assert.NotNull(capability);
        Assert.Equal(expectedState, capability.HdrState);
        Assert.Equal(2560, capability.LogicalSize?.Width);
        Assert.Equal(1440, capability.LogicalSize?.Height);
    }

    [Fact]
    public void ReturnsNoSnapshotWhenTargetSelectionIsUnavailable()
    {
        var provider = new WindowsTargetCapabilityProvider(
            () => throw new InvalidOperationException("must not resolve"),
            _ => throw new InvalidOperationException("must not probe"));

        var capability = provider.GetCurrent();

        Assert.Null(capability);
    }

    [Fact]
    public void NativeRegionPreservesNegativePhysicalMonitorOrigin()
    {
        var monitor = new MonitorHandle(
            (nint)42,
            @"\\.\DISPLAY2",
            Left: -3840,
            Top: -240,
            Width: 3840,
            Height: 2160,
            EffectiveDpiX: 144,
            EffectiveDpiY: 144);
        var provider = new WindowsTargetCapabilityProvider(
            () => monitor,
            _ => new HdrDisplayCapability(HdrDisplayState.Active, null, @"\\.\DISPLAY2"));

        var capability = provider.GetCurrent();

        Assert.NotNull(capability);
        Assert.True(capability.SupportsNativeRegionCapture);
        Assert.Equal(-3840, capability.PixelLeft);
        Assert.Equal(-240, capability.PixelTop);
        Assert.Equal(3840, capability.PixelWidth);
        Assert.Equal(2160, capability.PixelHeight);
    }

    [Fact]
    public void CalculatesTargetLocalLogicalSizeFromEffectiveDpi()
    {
        var monitor = new MonitorHandle(
            (nint)42,
            @"\\.\DISPLAY2",
            Width: 3840,
            Height: 2160,
            EffectiveDpiX: 144,
            EffectiveDpiY: 144);

        var logicalSize = WindowsDisplayTargetFactory.CalculateLogicalSize(3840, 2160, monitor);

        Assert.Equal(2560, logicalSize?.Width);
        Assert.Equal(1440, logicalSize?.Height);
    }

    [Fact]
    public void OmitsLogicalSizeWhenEffectiveDpiIsUnknown()
    {
        var monitor = new MonitorHandle((nint)42, @"\\.\DISPLAY2", Width: 3840, Height: 2160);

        var logicalSize = WindowsDisplayTargetFactory.CalculateLogicalSize(3840, 2160, monitor);

        Assert.Null(logicalSize);
    }
}
