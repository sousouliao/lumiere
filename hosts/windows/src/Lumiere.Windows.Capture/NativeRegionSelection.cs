namespace Lumiere.Windows.Capture;

/// <summary>
/// Projects a drag in target-local logical coordinates. The native window owns pointer
/// events; this class owns the selection rule shared by mouse input and crop delivery.
/// </summary>
internal sealed class NativeRegionSelection
{
    private const double MinimumWidth = 32;
    private const double MinimumHeight = 24;
    private readonly WindowsTargetLogicalSize target;
    private (double X, double Y)? origin;
    private (double X, double Y)? pointer;

    public NativeRegionSelection(WindowsTargetLogicalSize target)
    {
        ArgumentNullException.ThrowIfNull(target);
        if (!double.IsFinite(target.Width) || !double.IsFinite(target.Height)
            || target.Width <= 0 || target.Height <= 0)
        {
            throw new ArgumentOutOfRangeException(nameof(target));
        }

        this.target = target;
    }

    public WindowsRegionGeometry? Current =>
        origin is { } start && pointer is { } end
        && start.X != end.X && start.Y != end.Y
            ? Geometry(start, end)
            : null;

    public void Begin(double x, double y)
    {
        origin = Clamp(x, y);
        pointer = origin;
    }

    public void Update(double x, double y)
    {
        if (origin is not null)
        {
            pointer = Clamp(x, y);
        }
    }

    public WindowsRegionGeometry? Complete(double x, double y)
    {
        Update(x, y);
        var geometry = Current;
        origin = null;
        pointer = null;
        return geometry is { Width: >= MinimumWidth, Height: >= MinimumHeight }
            ? geometry
            : null;
    }

    public void Reset()
    {
        origin = null;
        pointer = null;
    }

    private (double X, double Y) Clamp(double x, double y)
    {
        if (!double.IsFinite(x) || !double.IsFinite(y))
        {
            throw new ArgumentOutOfRangeException(nameof(x), "Pointer coordinates must be finite.");
        }

        return (Math.Clamp(x, 0, target.Width), Math.Clamp(y, 0, target.Height));
    }

    private static WindowsRegionGeometry Geometry(
        (double X, double Y) start,
        (double X, double Y) end) =>
        new(
            Math.Min(start.X, end.X),
            Math.Min(start.Y, end.Y),
            Math.Abs(end.X - start.X),
            Math.Abs(end.Y - start.Y));
}
