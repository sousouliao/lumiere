using Lumiere.Windows.Graphics.Presentation;

namespace Lumiere.Windows.Capture;

/// <summary>
/// Presents a converted, full-pixel frozen frame and returns target-local logical geometry.
/// The caller retains ownership of the surface until selection has ended and the window is hidden.
/// </summary>
internal interface INativeRegionOverlay
{
    Task<WindowsRegionGeometry?> SelectAsync(
        CapturedFrameTexture visualMatchSurface,
        WindowsTargetCapability target,
        CancellationToken cancellationToken);
}
