using Lumiere.Windows.Graphics.Presentation;

namespace Lumiere.Windows.Capture;

/// <summary>
/// Presents a converted, full-pixel frozen frame. The window must be hidden before this
/// method returns, so the caller can release the surface or acquire another target.
/// </summary>
internal interface INativeRegionOverlay
{
    Task<NativeRegionOverlayResult> SelectAsync(
        CapturedFrameTexture visualMatchSurface,
        WindowsTargetCapability target,
        CancellationToken cancellationToken);
}

internal enum NativeRegionOverlayAction
{
    Selected,
    Cancelled,
    SwitchTarget,
}

internal sealed record NativeRegionOverlayResult(
    NativeRegionOverlayAction Action,
    WindowsRegionGeometry? Geometry = null)
{
    public static NativeRegionOverlayResult Selected(WindowsRegionGeometry geometry) =>
        new(NativeRegionOverlayAction.Selected, geometry);

    public static NativeRegionOverlayResult Cancelled { get; } =
        new(NativeRegionOverlayAction.Cancelled);

    public static NativeRegionOverlayResult SwitchTarget { get; } =
        new(NativeRegionOverlayAction.SwitchTarget);
}
