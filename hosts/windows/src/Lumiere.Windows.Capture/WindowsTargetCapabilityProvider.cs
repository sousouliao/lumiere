using Lumiere.Windows.Graphics.Hdr;
using Lumiere.Windows.Interop;
using Lumiere.Windows.Interop.Diagnostics;
using Microsoft.Extensions.Logging;

namespace Lumiere.Windows.Capture;

public enum WindowsTargetHdrState
{
    Unknown = 0,
    Active,
    Inactive,
}

public sealed record WindowsTargetLogicalSize(double Width, double Height);

public sealed class WindowsTargetCapability
{
    private readonly Func<CaptureTarget>? captureTargetFactory;

    public WindowsTargetCapability(
        WindowsTargetHdrState hdrState,
        WindowsTargetLogicalSize? logicalSize)
        : this(hdrState, logicalSize, pixelLeft: null, pixelTop: null,
            pixelWidth: null, pixelHeight: null, captureTargetFactory: null)
    {
    }

    internal WindowsTargetCapability(
        WindowsTargetHdrState hdrState,
        WindowsTargetLogicalSize? logicalSize,
        int? pixelLeft,
        int? pixelTop,
        int? pixelWidth,
        int? pixelHeight,
        Func<CaptureTarget>? captureTargetFactory)
    {
        HdrState = hdrState;
        LogicalSize = logicalSize;
        PixelLeft = pixelLeft;
        PixelTop = pixelTop;
        PixelWidth = pixelWidth;
        PixelHeight = pixelHeight;
        this.captureTargetFactory = captureTargetFactory;
    }

    public WindowsTargetHdrState HdrState { get; }

    public WindowsTargetLogicalSize? LogicalSize { get; }

    public bool SupportsRegionCapture =>
        LogicalSize is not null
        && PixelWidth is > 0
        && PixelHeight is > 0
        && captureTargetFactory is not null;

    internal bool SupportsNativeRegionCapture =>
        SupportsRegionCapture && PixelLeft is not null && PixelTop is not null;

    internal int? PixelLeft { get; }

    internal int? PixelTop { get; }

    internal int? PixelWidth { get; }

    internal int? PixelHeight { get; }

    internal CaptureTarget CreateCaptureTarget() =>
        captureTargetFactory?.Invoke()
        ?? throw new InvalidOperationException("The target snapshot cannot resolve a capture target.");

    internal static WindowsTargetCapability CreateForTest(
        WindowsTargetHdrState hdrState,
        WindowsTargetLogicalSize logicalSize,
        CaptureTarget target) =>
        new(
            hdrState,
            logicalSize,
            target.DisplayIdentity?.Left,
            target.DisplayIdentity?.Top,
            target.Size.Width,
            target.Size.Height,
            () => target);
}

public sealed class WindowsTargetCapabilityProvider
{
    private static readonly ILogger Logger = LumiereLoggerFactory.CreateLogger(LogCategories.Capture);
    private readonly Func<MonitorHandle> monitorResolver;
    private readonly Func<MonitorHandle, HdrDisplayCapability> probeHdrCapability;

    public WindowsTargetCapabilityProvider()
        : this(
            MonitorSelectionInterop.GetCurrentMonitorFromCursor,
            WindowsDisplayTargetFactory.ProbeHdrCapability)
    {
    }

    internal WindowsTargetCapabilityProvider(
        Func<MonitorHandle> monitorResolver,
        Func<MonitorHandle, HdrDisplayCapability> probeHdrCapability)
    {
        this.monitorResolver = monitorResolver ?? throw new ArgumentNullException(nameof(monitorResolver));
        this.probeHdrCapability = probeHdrCapability ?? throw new ArgumentNullException(nameof(probeHdrCapability));
    }

    public WindowsTargetCapability? GetCurrent()
    {
        try
        {
            var monitor = monitorResolver();
            var hdrCapability = probeHdrCapability(monitor);
            var logicalSize = monitor.Width is > 0 && monitor.Height is > 0
                ? WindowsDisplayTargetFactory.CalculateLogicalSize(
                    monitor.Width.Value,
                    monitor.Height.Value,
                    monitor)
                : null;
            return new WindowsTargetCapability(
                MapHdrState(hdrCapability.State),
                logicalSize is { } size
                    ? new WindowsTargetLogicalSize(size.Width, size.Height)
                    : null,
                monitor.Left,
                monitor.Top,
                monitor.Width,
                monitor.Height,
                () => WindowsDisplayTargetFactory.Create(monitor));
        }
        catch (Exception exception)
        {
            _ = InteropFailureDiagnostics.LogAndFormat(exception, Logger);
            return null;
        }
    }

    private static WindowsTargetHdrState MapHdrState(HdrDisplayState state) =>
        state switch
        {
            HdrDisplayState.Active => WindowsTargetHdrState.Active,
            HdrDisplayState.Inactive => WindowsTargetHdrState.Inactive,
            _ => WindowsTargetHdrState.Unknown,
        };
}
