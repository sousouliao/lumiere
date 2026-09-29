using Lumiere.Windows.Capture;
using Lumiere.Windows.Graphics.Hdr;
using Lumiere.Windows.Graphics.Output;
using Lumiere.Windows.Graphics.Presentation;
using Vortice.DXGI;
using Windows.Graphics;
using Xunit;

namespace Lumiere.Windows.Capture.Tests;

public sealed class WindowsDisplayCaptureEngineTests
{
    [Fact]
    public async Task CaptureDisplayAsync_OwnsFrameDeliveryAndSessionTeardown()
    {
        var sessionDisposed = false;
        var target = CreateTarget();
        await using var engine = CreateEngine(
            target,
            (onFrame, _) =>
            {
                onFrame(new CapturedFrameTexture(null, 2, 2, "test frame"));
                return CaptureStartResult.StartSucceeded(
                    new CaptureSessionResources(() => sessionDisposed = true),
                    EngineReadinessStatus.Initializing("Capture started"));
            });

        var result = await engine.CaptureDisplayAsync(
            new WindowsCaptureRequest("request-1", OutputTarget.Folder, "C:\\captures"));

        Assert.Equal(WindowsCaptureOutcome.Delivered, result.Outcome);
        Assert.True(result.HasDeliveredArtifact);
        Assert.True(sessionDisposed);
    }

    [Fact]
    public async Task CaptureDisplayAsync_CancellationDisposesTheActiveSession()
    {
        var sessionStarted = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var sessionDisposed = false;
        var target = CreateTarget();
        await using var engine = CreateEngine(
            target,
            (_, _) =>
            {
                sessionStarted.SetResult();
                return CaptureStartResult.StartSucceeded(
                    new CaptureSessionResources(() => sessionDisposed = true),
                    EngineReadinessStatus.Initializing("Capture started"));
            });
        using var cancellation = new CancellationTokenSource();

        var capture = engine.CaptureDisplayAsync(
            new WindowsCaptureRequest("request-2", OutputTarget.Folder, "C:\\captures"),
            cancellation.Token);
        await sessionStarted.Task;
        await cancellation.CancelAsync();
        var result = await capture;

        Assert.Equal(WindowsCaptureOutcome.Cancelled, result.Outcome);
        Assert.True(sessionDisposed);
    }

    [Fact]
    public async Task PrepareThenCommitRegion_CropsTheFrozenFrameWithoutRecapturing()
    {
        var target = CreateTarget();
        var output = new RecordingOutput();
        var previewEncoder = new SuccessfulPreviewEncoder();
        var frameArrivals = 0;
        await using var engine = CreateEngine(
            target,
            (onFrame, _) =>
            {
                frameArrivals++;
                onFrame(new CapturedFrameTexture(null, 2, 2, "test frame"));
                return CaptureStartResult.StartSucceeded(
                    new CaptureSessionResources(() => { }),
                    EngineReadinessStatus.Initializing("Capture started"));
            },
            output,
            previewEncoder: previewEncoder);
        var targetSnapshot = WindowsTargetCapability.CreateForTest(
            WindowsTargetHdrState.Inactive,
            new WindowsTargetLogicalSize(1, 1),
            target);

        var prepared = await engine.PrepareRegionAsync("prepare-region-test", targetSnapshot);
        Assert.True(prepared.Prepared);
        Assert.False(string.IsNullOrWhiteSpace(prepared.SessionId));
        Assert.True(File.Exists(prepared.PreviewPath));
        Assert.Equal(1, prepared.PreviewPixelWidth);
        Assert.Equal(1, prepared.PreviewPixelHeight);
        Assert.Equal((1, 1), previewEncoder.RequestedSize);
        Assert.Equal(1, frameArrivals);

        var result = await engine.CommitRegionAsync(
            prepared.SessionId!,
            new WindowsCaptureRequest("request-region", OutputTarget.Folder, "C:\\captures"),
            new WindowsRegionGeometry(0.25, 0.25, 0.5, 0.5));

        Assert.Equal(WindowsCaptureOutcome.Delivered, result.Outcome);
        Assert.Equal(new CropPixelRect(0, 0, 2, 2), output.Request?.CropRegion);
        Assert.Equal(1, frameArrivals);
        Assert.False(File.Exists(prepared.PreviewPath));

        var repeated = await engine.CommitRegionAsync(
            prepared.SessionId!,
            new WindowsCaptureRequest("request-region-repeat", OutputTarget.Folder, "C:\\captures"),
            new WindowsRegionGeometry(0.25, 0.25, 0.5, 0.5));
        Assert.Equal(WindowsCaptureOutcome.Unavailable, repeated.Outcome);
    }

    [Fact]
    public async Task NativeRegion_UsesOneFrozenFrameForPresentationAndDelivery()
    {
        var target = CreateTarget();
        var output = new RecordingOutput();
        var captures = 0;
        var rendered = 0;
        await using var engine = CreateEngine(
            target,
            (onFrame, _) =>
            {
                captures++;
                onFrame(new CapturedFrameTexture(null, 2, 2, "frozen"));
                return CaptureStartResult.StartSucceeded(
                    new CaptureSessionResources(() => { }),
                    EngineReadinessStatus.Initializing("Capture started"));
            },
            output,
            renderNativeRegion: (source, _) =>
            {
                Assert.Equal("frozen", source.SourceDescription);
                rendered++;
                return new CapturedFrameTexture(null, 2, 2, "visual match");
            });
        var overlay = new FakeNativeOverlay((surface, _, _) =>
        {
            Assert.Equal("visual match", surface.SourceDescription);
            return Task.FromResult(NativeRegionOverlayResult.Selected(
                new WindowsRegionGeometry(16, 12, 32, 24)));
        });

        var result = await engine.CaptureNativeRegionAsync(
            new WindowsCaptureRequest("native-region", OutputTarget.Folder, "C:\\captures"),
            () => RegionTarget(target),
            overlay);

        Assert.Equal(WindowsCaptureOutcome.Delivered, result.Outcome);
        Assert.Equal(new CropPixelRect(0, 0, 2, 2), output.Request?.CropRegion);
        Assert.Equal("frozen", output.Request?.Texture.SourceDescription);
        Assert.Equal(1, captures);
        Assert.Equal(1, rendered);
    }

    [Fact]
    public async Task NativeRegion_SwitchReleasesOldFrameAndCapturesLatestTarget()
    {
        var target = CreateTarget();
        var output = new RecordingOutput();
        var captures = 0;
        var targetResolutions = 0;
        var presentations = 0;
        await using var engine = CreateEngine(
            target,
            (onFrame, _) =>
            {
                captures++;
                onFrame(new CapturedFrameTexture(null, 2, 2, $"frame-{captures}"));
                return CaptureStartResult.StartSucceeded(
                    new CaptureSessionResources(() => { }),
                    EngineReadinessStatus.Initializing("Capture started"));
            },
            output,
            renderNativeRegion: (source, _) =>
                new CapturedFrameTexture(null, 2, 2, source.SourceDescription));
        var overlay = new FakeNativeOverlay((surface, _, _) =>
        {
            presentations++;
            Assert.Equal($"frame-{presentations}", surface.SourceDescription);
            return Task.FromResult(presentations == 1
                ? NativeRegionOverlayResult.SwitchTarget
                : NativeRegionOverlayResult.Selected(new WindowsRegionGeometry(16, 12, 32, 24)));
        });

        var result = await engine.CaptureNativeRegionAsync(
            new WindowsCaptureRequest("native-switch", OutputTarget.Folder, "C:\\captures"),
            () =>
            {
                targetResolutions++;
                return RegionTarget(target);
            },
            overlay);

        Assert.Equal(WindowsCaptureOutcome.Delivered, result.Outcome);
        Assert.Equal("frame-2", output.Request?.Texture.SourceDescription);
        Assert.Equal(2, captures);
        Assert.Equal(2, presentations);
        Assert.Equal(2, targetResolutions);
    }

    [Fact]
    public async Task NativeRegion_CancellationReleasesSelectionAndRejectsConcurrentCapture()
    {
        var target = CreateTarget();
        var selecting = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var overlay = new FakeNativeOverlay(async (_, _, token) =>
        {
            selecting.SetResult();
            await Task.Delay(Timeout.Infinite, token);
            return NativeRegionOverlayResult.Cancelled;
        });
        await using var engine = CreateEngine(
            target,
            (onFrame, _) =>
            {
                onFrame(new CapturedFrameTexture(null, 2, 2, "frozen"));
                return CaptureStartResult.StartSucceeded(
                    new CaptureSessionResources(() => { }),
                    EngineReadinessStatus.Initializing("Capture started"));
            },
            renderNativeRegion: (_, _) => new CapturedFrameTexture(null, 2, 2, "visual match"));
        using var cancellation = new CancellationTokenSource();
        var capture = engine.CaptureNativeRegionAsync(
            new WindowsCaptureRequest("native-cancel", OutputTarget.Folder, "C:\\captures"),
            () => RegionTarget(target),
            overlay,
            cancellation.Token);
        await selecting.Task;

        var competing = await engine.CaptureDisplayAsync(
            new WindowsCaptureRequest("competing", OutputTarget.Folder, "C:\\captures"));
        Assert.Equal(WindowsCaptureOutcome.Unavailable, competing.Outcome);

        await cancellation.CancelAsync();
        Assert.Equal(WindowsCaptureOutcome.Cancelled, (await capture).Outcome);
        Assert.Equal(WindowsCaptureOutcome.Delivered,
            (await engine.CaptureDisplayAsync(
                new WindowsCaptureRequest("after-cancel", OutputTarget.Folder, "C:\\captures"))).Outcome);
    }

    [Fact]
    public async Task NativeRegion_EngineExitCancelsSelectionBeforeDisposingResources()
    {
        var target = CreateTarget();
        var selecting = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        var selectionEnded = false;
        var rendererDisposedAfterSelection = false;
        var overlay = new FakeNativeOverlay(async (_, _, token) =>
        {
            selecting.SetResult();
            try
            {
                await Task.Delay(Timeout.Infinite, token);
            }
            finally
            {
                selectionEnded = true;
            }
            return NativeRegionOverlayResult.Cancelled;
        });
        var engine = CreateEngine(
            target,
            (onFrame, _) =>
            {
                onFrame(new CapturedFrameTexture(null, 2, 2, "frozen"));
                return CaptureStartResult.StartSucceeded(
                    new CaptureSessionResources(() => { }),
                    EngineReadinessStatus.Initializing("Capture started"));
            },
            renderNativeRegion: (_, _) => new CapturedFrameTexture(null, 2, 2, "visual match"),
            nativeRegionRenderer: new ActionDisposable(
                () => rendererDisposedAfterSelection = selectionEnded));

        var capture = engine.CaptureNativeRegionAsync(
            new WindowsCaptureRequest("native-exit", OutputTarget.Folder, "C:\\captures"),
            () => RegionTarget(target),
            overlay);
        await selecting.Task;
        await engine.DisposeAsync();

        Assert.Equal(WindowsCaptureOutcome.Cancelled, (await capture).Outcome);
        Assert.True(rendererDisposedAfterSelection);
    }

    [Fact]
    public async Task CaptureDisplayAsync_MapsTargetResolutionFailureToUnavailable()
    {
        await using var engine = new WindowsDisplayCaptureEngine(
            command => CaptureCommandResult.Accepted(command),
            _ => { },
            _ => Task.FromResult(CaptureTargetSelectionResult.Failed(
                EngineReadinessStatus.Failed(
                    EngineReadinessStage.Interop,
                    "Capture target selection failed",
                    "GetCursorPos returned access denied."))),
            _ => throw new InvalidOperationException("must not probe"),
            (_, _, _) => throw new InvalidOperationException("must not capture"),
            new SuccessfulOutput(),
            new SuccessfulPreviewEncoder());

        var result = await engine.CaptureDisplayAsync(
            new WindowsCaptureRequest("request-unavailable", OutputTarget.Folder, "C:\\captures"));

        Assert.Equal(WindowsCaptureOutcome.Unavailable, result.Outcome);
    }

    [Fact]
    public async Task CaptureDisplayAsync_MapsOutputExceptionToCaptureFailure()
    {
        var target = CreateTarget();
        await using var engine = CreateEngine(
            target,
            (onFrame, _) =>
            {
                onFrame(new CapturedFrameTexture(null, 2, 2, "test frame"));
                return CaptureStartResult.StartSucceeded(
                    new CaptureSessionResources(() => { }),
                    EngineReadinessStatus.Initializing("Capture started"));
            },
            new ThrowingOutput());

        var result = await engine.CaptureDisplayAsync(
            new WindowsCaptureRequest("request-output-failure", OutputTarget.Folder, "C:\\captures"));

        Assert.Equal(WindowsCaptureOutcome.Failed, result.Outcome);
        Assert.Null(result.Output);
        Assert.False(result.HasDeliveredArtifact);
    }

    [Fact]
    public async Task CaptureDisplayAsync_NormalizesUsingTheMatchedHdrTargetSdrWhiteLevel()
    {
        var target = CreateTarget();
        var output = new RecordingOutput();
        await using var engine = CreateEngine(
            target,
            (onFrame, _) =>
            {
                onFrame(new CapturedFrameTexture(null, 2, 2, "test frame"));
                return CaptureStartResult.StartSucceeded(
                    new CaptureSessionResources(() => { }),
                    EngineReadinessStatus.Initializing("Capture started"));
            },
            output,
            new HdrDisplayCapability(
                HdrDisplayState.Active,
                ColorSpaceType.RgbFullG2084NoneP2020,
                "test display",
                HdrDisplayMatchKind.DeviceName,
                SdrWhiteLevelInNits: 240f));

        var result = await engine.CaptureDisplayAsync(
            new WindowsCaptureRequest("request-hdr", OutputTarget.Folder, "C:\\captures"));

        Assert.Equal(WindowsCaptureOutcome.Delivered, result.Outcome);
        Assert.Equal(80f / 240f, output.Request?.VisualMatchContext.InputLinearScale);
    }

    [Fact]
    public async Task CaptureDisplayAsync_FailsInsteadOfClaimingVisualMatchWhenHdrWhiteLevelIsUnavailable()
    {
        var target = CreateTarget();
        var output = new RecordingOutput();
        await using var engine = CreateEngine(
            target,
            (onFrame, _) =>
            {
                onFrame(new CapturedFrameTexture(null, 2, 2, "test frame"));
                return CaptureStartResult.StartSucceeded(
                    new CaptureSessionResources(() => { }),
                    EngineReadinessStatus.Initializing("Capture started"));
            },
            output,
            new HdrDisplayCapability(
                HdrDisplayState.Active,
                ColorSpaceType.RgbFullG2084NoneP2020,
                "test display"));

        var result = await engine.CaptureDisplayAsync(
            new WindowsCaptureRequest("request-hdr-unvalidated", OutputTarget.Folder, "C:\\captures"));

        Assert.Equal(WindowsCaptureOutcome.Failed, result.Outcome);
        Assert.Null(result.Output);
        Assert.Null(output.Request);
    }

    private static WindowsDisplayCaptureEngine CreateEngine(
        CaptureTarget target,
        Func<Action<CapturedFrameTexture>, Action<EngineReadinessStatus>, CaptureStartResult> startCapture,
        IOutputService? output = null,
        HdrDisplayCapability? hdrCapability = null,
        IRegionPreviewEncoder? previewEncoder = null,
        Func<CapturedFrameTexture, SrgbVisualMatchConversionContext, CapturedFrameTexture>? renderNativeRegion = null,
        IDisposable? nativeRegionRenderer = null) =>
        new(
            command => CaptureCommandResult.Accepted(command),
            _ => { },
            _ => Task.FromResult(CaptureTargetSelectionResult.Selected(
                target,
                EngineReadinessStatus.Initializing("Target selected"))),
            _ => hdrCapability ?? new HdrDisplayCapability(
                HdrDisplayState.Inactive,
                ColorSpaceType.RgbFullG22NoneP709,
                "test display"),
            (_, onFrame, onFailure) => startCapture(onFrame, onFailure),
            output ?? new SuccessfulOutput(),
            previewEncoder ?? new SuccessfulPreviewEncoder(),
            renderNativeRegion: renderNativeRegion,
            nativeRegionRenderer: nativeRegionRenderer);

    private static WindowsTargetCapability RegionTarget(CaptureTarget target) =>
        WindowsTargetCapability.CreateForTest(
            WindowsTargetHdrState.Inactive,
            new WindowsTargetLogicalSize(64, 48),
            target);

    private static CaptureTarget CreateTarget() =>
        CaptureTarget.CreateForTest(
            new SizeInt32 { Width = 2, Height = 2 },
            "test display",
            CaptureTargetKind.Display,
            new DisplayOutputIdentity("test display", 0, 0, 2, 2));

    private sealed class SuccessfulOutput : IOutputService
    {
        public Task<OutputResult> ExecuteOutputAsync(
            OutputRequest request,
            CancellationToken cancellationToken = default) =>
            Task.FromResult(OutputResult.FromTargets(
                OutputTargetResult.Success(
                    OutputTarget.Folder,
                    "Saved to folder",
                    artifactPath: "C:\\captures\\test.png")));

    }

    private sealed class RecordingOutput : IOutputService
    {
        public OutputRequest? Request { get; private set; }

        public Task<OutputResult> ExecuteOutputAsync(
            OutputRequest request,
            CancellationToken cancellationToken = default)
        {
            Request = request;
            return Task.FromResult(OutputResult.FromTargets(
                OutputTargetResult.Success(
                    OutputTarget.Folder,
                    "Saved to folder",
                    artifactPath: "C:\\captures\\test.png")));
        }

    }

    private sealed class ThrowingOutput : IOutputService
    {
        public Task<OutputResult> ExecuteOutputAsync(
            OutputRequest request,
            CancellationToken cancellationToken = default) =>
            throw new InvalidOperationException("encoding failed");

    }

    private sealed class SuccessfulPreviewEncoder : IRegionPreviewEncoder
    {
        public (int Width, int Height)? RequestedSize { get; private set; }

        public Task<RegionPreviewArtifact> EncodePreviewAsync(
            CapturedFrameTexture texture,
            int outputWidth,
            int outputHeight,
            SrgbVisualMatchConversionContext context,
            CancellationToken cancellationToken = default)
        {
            RequestedSize = (outputWidth, outputHeight);
            return Task.FromResult(new RegionPreviewArtifact(PreviewPng, outputWidth, outputHeight));
        }
    }

    private sealed class FakeNativeOverlay(
        Func<CapturedFrameTexture, WindowsTargetCapability, CancellationToken,
            Task<NativeRegionOverlayResult>> select) : INativeRegionOverlay
    {
        public Task<NativeRegionOverlayResult> SelectAsync(
            CapturedFrameTexture visualMatchSurface,
            WindowsTargetCapability target,
            CancellationToken cancellationToken) =>
            select(visualMatchSurface, target, cancellationToken);
    }

    private sealed class ActionDisposable(Action action) : IDisposable
    {
        public void Dispose() => action();
    }

    private static readonly byte[] PreviewPng = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
}
