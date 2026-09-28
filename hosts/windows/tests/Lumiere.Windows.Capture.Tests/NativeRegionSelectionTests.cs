using Lumiere.Windows.Capture;
using Xunit;

namespace Lumiere.Windows.Capture.Tests;

public sealed class NativeRegionSelectionTests
{
    [Fact]
    public void ReverseDragProjectsIntoTargetLocalGeometry()
    {
        var selection = new NativeRegionSelection(new WindowsTargetLogicalSize(2560, 1440));
        selection.Begin(450, 240);
        selection.Update(150, 90);

        Assert.Equal(new WindowsRegionGeometry(150, 90, 300, 150), selection.Current);
        Assert.Equal(new WindowsRegionGeometry(150, 90, 300, 150), selection.Complete(150, 90));
        Assert.Null(selection.Current);
    }

    [Fact]
    public void MinimumSelectionIsInclusiveAndPointerIsClampedToTarget()
    {
        var selection = new NativeRegionSelection(new WindowsTargetLogicalSize(100, 80));
        selection.Begin(100, 80);
        Assert.Equal(new WindowsRegionGeometry(68, 56, 32, 24), selection.Complete(68, 56));

        selection.Begin(68, 56);
        Assert.Equal(new WindowsRegionGeometry(68, 56, 32, 24), selection.Complete(1000, 1000));

        selection.Begin(0, 0);
        Assert.Null(selection.Complete(31, 24));
        selection.Begin(0, 0);
        Assert.Null(selection.Complete(32, 23));
    }

    [Fact]
    public void CancelledOrEmptyDragCannotCommit()
    {
        var selection = new NativeRegionSelection(new WindowsTargetLogicalSize(100, 80));
        Assert.Null(selection.Complete(80, 60));
        selection.Begin(10, 10);
        selection.Reset();
        Assert.Null(selection.Complete(70, 50));
        selection.Begin(10, 10);
        Assert.Null(selection.Complete(10, 70));
    }
}
