using Lumiere.Windows.Graphics.Devices;
using Lumiere.Windows.Graphics.Hdr;
using Lumiere.Windows.Graphics.Output;
using Lumiere.Windows.Graphics.Presentation;
using Lumiere.Windows.Interop;
using Lumiere.Windows.Interop.Diagnostics;
using Microsoft.Extensions.Logging;

namespace Lumiere.Windows.Capture;

/// <summary>
/// Owns one complete capture operation, from target resolution through delivery and teardown.
/// The platform Host maps protocol requests to this interface and never owns native frame resources.
/// </summary>
public sealed class WindowsDisplayCaptureEngine : IAsyncDisposable
{
    private static readonly Lazy<ILogger> LoggerHolder = new(
        () => LumiereLoggerFactory.CreateLogger(LogCategories.Capture));
    private static readonly TimeSpan DefaultFrameTimeout = TimeSpan.FromSeconds(10);

    private readonly Func<CaptureCommand, CaptureCommandResult> reserveCommand;
    private readonly Action<CaptureSessionState> updateSessionState;
    private readonly Func<CancellationToken, Task<CaptureTargetSelectionResult>> selectTargetAsync;
    private readonly Func<CaptureTarget, HdrDisplayCapability> probeHdrCapability;
    private readonly Func<
        CaptureTarget,
        Action<CapturedFrameTexture>,
        Action<EngineReadinessStatus>,
        CaptureStartResult> startCapture;
    private static readonly int RegionLeaseMilliseconds = 60_000;
    private readonly IOutputService output;
    private readonly IRegionPreviewEncoder regionPreviewEncoder;
    private readonly Func<CapturedFrameTexture, CapturedFrameTexture> copyOwnedFrame;
    private readonly Func<CapturedFrameTexture, SrgbVisualMatchConversionContext, CapturedFrameTexture>? renderNativeRegion;
    private readonly IDisposable? nativeRegionRenderer;
    private readonly INativeRegionOverlay? nativeRegionOverlay;
    private readonly IDisposable? ownedResources;
    private readonly TimeSpan frameTimeout;
    private readonly SemaphoreSlim operationGate = new(1, 1);
    private readonly SemaphoreSlim nativeRegionGate = new(1, 1);
    private readonly CancellationTokenSource lifetimeCancellation = new();
    private readonly object frozenLock = new();
    private FrozenRegionSession? frozenRegion;
    private int nativeRegionActive;
    private int disposed;

    internal WindowsDisplayCaptureEngine(
        Func<CaptureCommand, CaptureCommandResult> reserveCommand,
        Action<CaptureSessionState> updateSessionState,
        Func<CancellationToken, Task<CaptureTargetSelectionResult>> selectTargetAsync,
        Func<CaptureTarget, HdrDisplayCapability> probeHdrCapability,
        Func<CaptureTarget, Action<CapturedFrameTexture>, Action<EngineReadinessStatus>, CaptureStartResult> startCapture,
        IOutputService output,
        IRegionPreviewEncoder regionPreviewEncoder,
        IDisposable? ownedResources = null,
        TimeSpan? frameTimeout = null,
        Func<CapturedFrameTexture, CapturedFrameTexture>? copyOwnedFrame = null,
        Func<CapturedFrameTexture, SrgbVisualMatchConversionContext, CapturedFrameTexture>? renderNativeRegion = null,
        IDisposable? nativeRegionRenderer = null,
        INativeRegionOverlay? nativeRegionOverlay = null)
    {
        this.reserveCommand = reserveCommand ?? throw new ArgumentNullException(nameof(reserveCommand));
        this.updateSessionState = updateSessionState ?? throw new ArgumentNullException(nameof(updateSessionState));
        this.selectTargetAsync = selectTargetAsync ?? throw new ArgumentNullException(nameof(selectTargetAsync));
        this.probeHdrCapability = probeHdrCapability ?? throw new ArgumentNullException(nameof(probeHdrCapability));
        this.startCapture = startCapture ?? throw new ArgumentNullException(nameof(startCapture));
        this.output = output ?? throw new ArgumentNullException(nameof(output));
        this.regionPreviewEncoder = regionPreviewEncoder ?? throw new ArgumentNullException(nameof(regionPreviewEncoder));
        this.copyOwnedFrame = copyOwnedFrame ?? (frame => frame);
        this.renderNativeRegion = renderNativeRegion;
        this.nativeRegionRenderer = nativeRegionRenderer;
        this.nativeRegionOverlay = nativeRegionOverlay;
        this.ownedResources = ownedResources;
        this.frameTimeout = frameTimeout ?? DefaultFrameTimeout;
        if (this.frameTimeout <= TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(nameof(frameTimeout), this.frameTimeout, "Frame timeout must be positive.");
        }
    }

    public static async Task<WindowsDisplayCaptureEngine> CreateDefaultAsync(
        CancellationToken cancellationToken = default)
    {
        var borderOptions = await CaptureBorderAccess.ResolveAsync(cancellationToken);
        return CreateDefault(borderOptions);
    }

    internal static WindowsDisplayCaptureEngine CreateDefault(CaptureBorderOptions borderOptions)
    {
        ArgumentNullException.ThrowIfNull(borderOptions);
        var deviceResources = new GraphicsDeviceProvider().CreateDevice();
        GpuVisualMatchSurface? nativeRenderer = null;
        NativeRegionOverlayWindow? nativeOverlay = null;
        try
        {
            var captureService = new CaptureService(deviceResources, borderOptions);
            var targetSelection = new DirectMonitorCaptureTargetSelectionService(
                MonitorSelectionInterop.GetCurrentMonitorFromCursor,
                WindowsDisplayTargetFactory.Create);
            var output = ConfiguredOutputService.CreateDefault(deviceResources);
            var regionPreviewEncoder = new SrgbRegionPreviewEncoder(
                new CapturedFrameTextureReadback(deviceResources));
            nativeRenderer = new GpuVisualMatchSurface(deviceResources);
            nativeOverlay = new NativeRegionOverlayWindow(deviceResources);

            return new WindowsDisplayCaptureEngine(
                captureService.TryReserveCommand,
                captureService.UpdateSessionState,
                targetSelection.SelectTargetAsync,
                WindowsDisplayTargetFactory.ProbeHdrCapability,
                captureService.StartCapture,
                output,
                regionPreviewEncoder,
                deviceResources,
                copyOwnedFrame: captureService.CopyToOwnedTexture,
                renderNativeRegion: nativeRenderer.Render,
                nativeRegionRenderer: nativeRenderer,
                nativeRegionOverlay: nativeOverlay);
        }
        catch
        {
            nativeOverlay?.Dispose();
            nativeRenderer?.Dispose();
            deviceResources.Dispose();
            throw;
        }
    }

    public static void ConfigureLogging(ILoggerFactory loggerFactory) =>
        LumiereLoggerFactory.Configure(loggerFactory);

    public async Task<WindowsCaptureResult> CaptureDisplayAsync(
        WindowsCaptureRequest request,
        CancellationToken cancellationToken = default)
    {
        if (HasFrozenRegion() || Volatile.Read(ref nativeRegionActive) != 0)
        {
            return new WindowsCaptureResult(
                WindowsCaptureOutcome.Unavailable,
                "Region capture is already in progress",
                "A frozen Region capture is still active.");
        }

        var acquired = await CaptureAsync(request, freezeTarget: null, cancellationToken);
        return acquired.Result;
    }

    public async Task<WindowsPrepareRegionResult> PrepareRegionAsync(
        string requestId,
        WindowsTargetCapability target,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(requestId);
        ArgumentNullException.ThrowIfNull(target);
        if (Volatile.Read(ref nativeRegionActive) != 0)
        {
            return new WindowsPrepareRegionResult(
                false,
                WindowsCaptureOutcome.Unavailable,
                "Region capture is already in progress",
                "A native Region capture is still active.");
        }
        var startedAt = System.Diagnostics.Stopwatch.GetTimestamp();
        var lastAt = startedAt;
        await ReleaseFrozenRegionAsync();
        ReportRegionTiming(requestId, "target-resolved", startedAt, ref lastAt);
        var request = new WindowsCaptureRequest("prepare-region", OutputTarget.Folder);
        var acquired = await CaptureAsync(request, target, cancellationToken);
        if (acquired.HeldFrame is null)
        {
            return new WindowsPrepareRegionResult(
                false,
                acquired.Result.Outcome,
                acquired.Result.UserMessage,
                acquired.Result.TechnicalDetail);
        }
        ReportRegionTiming(
            requestId,
            "frame-acquired",
            startedAt,
            ref lastAt,
            acquired.HeldFrame.Texture.Width,
            acquired.HeldFrame.Texture.Height);

        try
        {
            var logicalSize = target.LogicalSize
                ?? throw new InvalidOperationException("Region capture requires a logical target size.");
            var previewWidth = checked((int)Math.Round(logicalSize.Width, MidpointRounding.AwayFromZero));
            var previewHeight = checked((int)Math.Round(logicalSize.Height, MidpointRounding.AwayFromZero));
            var previewStartedAt = System.Diagnostics.Stopwatch.GetTimestamp();
            var preview = await regionPreviewEncoder.EncodePreviewAsync(
                acquired.HeldFrame.Texture,
                previewWidth,
                previewHeight,
                ResolveVisualMatchContext(acquired.HeldFrame.HdrCapability),
                cancellationToken);
            var previewBaseElapsed = (long)Math.Round(
                System.Diagnostics.Stopwatch.GetElapsedTime(startedAt, previewStartedAt).TotalMilliseconds);
            LogRegionTiming(
                requestId,
                "preview-rendered",
                previewBaseElapsed + preview.RenderMilliseconds,
                preview.RenderMilliseconds,
                preview.Width,
                preview.Height);
            LogRegionTiming(
                requestId,
                "preview-encoded",
                previewBaseElapsed + preview.RenderMilliseconds + preview.EncodeMilliseconds,
                preview.EncodeMilliseconds,
                preview.Width,
                preview.Height,
                preview.Bytes.Length);
            lastAt = System.Diagnostics.Stopwatch.GetTimestamp();
            var previewPath = WriteRegionPreview(preview.Bytes);
            ReportRegionTiming(
                requestId,
                "preview-written",
                startedAt,
                ref lastAt,
                preview.Width,
                preview.Height,
                preview.Bytes.Length);
            var sessionId = Guid.NewGuid().ToString("N");
            var session = new FrozenRegionSession(
                sessionId,
                acquired.HeldFrame,
                previewPath,
                new CancellationTokenSource(TimeSpan.FromMilliseconds(RegionLeaseMilliseconds)));
            session.Lease.Token.Register(() =>
            {
                if (Volatile.Read(ref disposed) != 0)
                {
                    return;
                }

                _ = ReleaseFrozenRegionAsync(sessionId);
            });
            lock (frozenLock)
            {
                frozenRegion = session;
            }

            return new WindowsPrepareRegionResult(
                true,
                WindowsCaptureOutcome.Delivered,
                "Frozen region frame is ready.",
                $"Prepared {acquired.HeldFrame.Texture.Width}x{acquired.HeldFrame.Texture.Height} frozen frame.",
                sessionId,
                target.LogicalSize,
                previewPath,
                preview.Width,
                preview.Height,
                RegionLeaseMilliseconds);
        }
        catch
        {
            acquired.HeldFrame.Dispose();
            throw;
        }
    }

    private static void ReportRegionTiming(
        string requestId,
        string stage,
        long startedAt,
        ref long lastAt,
        int? width = null,
        int? height = null,
        int? bytes = null,
        long? stageMilliseconds = null)
    {
        var now = System.Diagnostics.Stopwatch.GetTimestamp();
        var elapsed = (long)Math.Round(
            System.Diagnostics.Stopwatch.GetElapsedTime(startedAt, now).TotalMilliseconds);
        var adjacent = stageMilliseconds ?? (long)Math.Round(
            System.Diagnostics.Stopwatch.GetElapsedTime(lastAt, now).TotalMilliseconds);
        lastAt = now;
        LogRegionTiming(requestId, stage, elapsed, adjacent, width, height, bytes);
    }

    private static void LogRegionTiming(
        string requestId,
        string stage,
        long elapsedMilliseconds,
        long stageMilliseconds,
        int? width = null,
        int? height = null,
        int? bytes = null)
    {
        LoggerHolder.Value.LogInformation(
            "region-capture-timing requestId={RequestId} stage={Stage} elapsedMilliseconds={ElapsedMilliseconds} stageMilliseconds={StageMilliseconds} width={Width} height={Height} bytes={Bytes}",
            requestId,
            stage,
            elapsedMilliseconds,
            stageMilliseconds,
            width,
            height,
            bytes);
    }

    public async Task<WindowsCaptureResult> CommitRegionAsync(
        string sessionId,
        WindowsCaptureRequest request,
        WindowsRegionGeometry geometry,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(sessionId);
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(geometry);
        if (Volatile.Read(ref nativeRegionActive) != 0)
        {
            return new WindowsCaptureResult(
                WindowsCaptureOutcome.Unavailable,
                "Region capture is already in progress",
                "A native Region capture is still active.");
        }

        FrozenRegionSession? session;
        lock (frozenLock)
        {
            session = frozenRegion;
            if (session is null || session.Id != sessionId)
            {
                return new WindowsCaptureResult(
                    WindowsCaptureOutcome.Unavailable,
                    "Region capture is unavailable",
                    "The frozen Region capture expired. Select the region again.");
            }

            frozenRegion = null;
        }

        session.Lease.Dispose();
        using var held = session.Frame;
        TryDeletePreview(session.PreviewPath);
        try
        {
            cancellationToken.ThrowIfCancellationRequested();
            var cropRegion = RegionCropResolver.Resolve(geometry, held.Target, held.CaptureTarget);
            return await DeliverHeldFrameAsync(
                request,
                held.Texture,
                held.HdrCapability,
                cropRegion,
                cancellationToken);
        }
        catch (ArgumentOutOfRangeException exception)
        {
            return new WindowsCaptureResult(
                WindowsCaptureOutcome.Unavailable,
                "Region capture is unavailable",
                exception.Message);
        }
        catch (InvalidOperationException exception)
        {
            return new WindowsCaptureResult(
                WindowsCaptureOutcome.Unavailable,
                "Region capture is unavailable",
                exception.Message);
        }
        catch (OperationCanceledException)
        {
            return Cancelled("Capture was cancelled by the caller.");
        }
    }

    public Task ReleaseRegionAsync(string sessionId) =>
        ReleaseFrozenRegionAsync(sessionId);

    internal async Task<WindowsCaptureResult> CaptureNativeRegionAsync(
        WindowsCaptureRequest request,
        Func<WindowsTargetCapability?> getCurrentTarget,
        INativeRegionOverlay overlay,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(request);
        ArgumentNullException.ThrowIfNull(getCurrentTarget);
        ArgumentNullException.ThrowIfNull(overlay);
        ThrowIfDisposed();
        if (renderNativeRegion is null)
        {
            throw new InvalidOperationException("The native Region renderer is unavailable.");
        }

        if (!await nativeRegionGate.WaitAsync(0, cancellationToken))
        {
            return new WindowsCaptureResult(
                WindowsCaptureOutcome.Unavailable,
                "Region capture is already in progress",
                "A native Region capture is still active.");
        }

        Volatile.Write(ref nativeRegionActive, 1);
        try
        {
            ThrowIfDisposed();
            using var sessionCancellation = CancellationTokenSource.CreateLinkedTokenSource(
                cancellationToken,
                lifetimeCancellation.Token);
            var sessionToken = sessionCancellation.Token;
            if (HasFrozenRegion())
            {
                return new WindowsCaptureResult(
                    WindowsCaptureOutcome.Unavailable,
                    "Region capture is unavailable",
                    "A frozen Region capture is still active.");
            }

            while (true)
            {
                sessionToken.ThrowIfCancellationRequested();
                var target = getCurrentTarget();
                if (target?.SupportsNativeRegionCapture != true)
                {
                    return new WindowsCaptureResult(
                        WindowsCaptureOutcome.Unavailable,
                        "Region capture is unavailable",
                        "The current target cannot start a native Region capture.");
                }
                LoggerHolder.Value.LogInformation(
                    "operation=NativeRegionTarget, correlation={CorrelationId}, logicalWidth={LogicalWidth}, logicalHeight={LogicalHeight}, pixelWidth={PixelWidth}, pixelHeight={PixelHeight}, left={Left}, top={Top}",
                    request.CorrelationId,
                    target.LogicalSize!.Width,
                    target.LogicalSize.Height,
                    target.PixelWidth,
                    target.PixelHeight,
                    target.PixelLeft,
                    target.PixelTop);

                var acquired = await CaptureAsync(request, target, sessionToken, isNativeRegion: true);
                if (acquired.HeldFrame is not { } held)
                {
                    return acquired.Result;
                }

                // Each frozen frame gets its own selection lease after acquisition.
                using (held)
                {
                    NativeRegionOverlayResult selection;
                    using (var lease = CancellationTokenSource.CreateLinkedTokenSource(sessionToken))
                    {
                        lease.CancelAfter(RegionLeaseMilliseconds);
                        using var surface = renderNativeRegion(
                            held.Texture,
                            ResolveVisualMatchContext(held.HdrCapability));
                        selection = await overlay.SelectAsync(surface, target, lease.Token);
                        if (lease.IsCancellationRequested)
                        {
                            return Cancelled("Region selection was cancelled or expired.");
                        }
                    }

                    if (selection.Action == NativeRegionOverlayAction.SwitchTarget)
                    {
                        continue;
                    }
                    if (selection is not { Action: NativeRegionOverlayAction.Selected, Geometry: { } geometry })
                    {
                        return Cancelled("Region selection was cancelled.");
                    }

                    CropPixelRect crop;
                    try
                    {
                        crop = RegionCropResolver.Resolve(geometry, held.Target, held.CaptureTarget);
                    }
                    catch (ArgumentException exception)
                    {
                        return new WindowsCaptureResult(
                            WindowsCaptureOutcome.Unavailable,
                            "Region capture is unavailable",
                            exception.Message);
                    }
                    catch (InvalidOperationException exception)
                    {
                        return new WindowsCaptureResult(
                            WindowsCaptureOutcome.Unavailable,
                            "Region capture is unavailable",
                            exception.Message);
                    }
                    LoggerHolder.Value.LogInformation(
                        "operation=NativeRegionCrop, correlation={CorrelationId}, logicalX={X}, logicalY={Y}, logicalWidth={Width}, logicalHeight={Height}, pixelLeft={PixelLeft}, pixelTop={PixelTop}, pixelWidth={PixelWidth}, pixelHeight={PixelHeight}",
                        request.CorrelationId,
                        geometry.X,
                        geometry.Y,
                        geometry.Width,
                        geometry.Height,
                        crop.X,
                        crop.Y,
                        crop.Width,
                        crop.Height);
                    return await DeliverHeldFrameAsync(
                        request,
                        held.Texture,
                        held.HdrCapability,
                        crop,
                        sessionToken);
                }
            }
        }
        catch (OperationCanceledException)
        {
            return Cancelled("Region selection was cancelled or expired.");
        }
        finally
        {
            Volatile.Write(ref nativeRegionActive, 0);
            nativeRegionGate.Release();
        }
    }

    internal Task<WindowsCaptureResult> CaptureNativeRegionAsync(
        WindowsCaptureRequest request,
        Func<WindowsTargetCapability?> getCurrentTarget,
        CancellationToken cancellationToken = default) =>
        CaptureNativeRegionAsync(
            request,
            getCurrentTarget,
            nativeRegionOverlay ?? throw new InvalidOperationException("Native Region overlay is unavailable."),
            cancellationToken);

    private async Task<CaptureAcquireResult> CaptureAsync(
        WindowsCaptureRequest request,
        WindowsTargetCapability? freezeTarget,
        CancellationToken cancellationToken,
        bool isNativeRegion = false)
    {
        ArgumentNullException.ThrowIfNull(request);
        ThrowIfDisposed();

        using var operationCancellation = CancellationTokenSource.CreateLinkedTokenSource(
            cancellationToken,
            lifetimeCancellation.Token);
        var token = operationCancellation.Token;

        try
        {
            await operationGate.WaitAsync(token);
        }
        catch (OperationCanceledException)
        {
            return new CaptureAcquireResult(Cancelled("Capture was cancelled before it started."), null);
        }

        try
        {
            ThrowIfDisposed();
            if (!isNativeRegion && Volatile.Read(ref nativeRegionActive) != 0)
            {
                return new CaptureAcquireResult(
                    new WindowsCaptureResult(
                        WindowsCaptureOutcome.Unavailable,
                        "Region capture is already in progress",
                        "A native Region capture is still active."),
                    null);
            }
            using var diagnosticScope = SessionDiagnosticScope.Begin(
                LoggerHolder.Value,
                correlationId: request.CorrelationId);

            var command = freezeTarget is null
                ? CaptureCommand.Fullscreen()
                : CaptureCommand.Region();
            var reservation = reserveCommand(command);
            if (!reservation.IsAccepted)
            {
                return new CaptureAcquireResult(
                    new WindowsCaptureResult(
                        WindowsCaptureOutcome.Failed,
                        "Capture is already active",
                        reservation.Readiness?.TechnicalDetail ?? "The Windows capture engine rejected the request."),
                    null);
            }

            return freezeTarget is null
                ? await CaptureReservedDisplayAsync(request, token)
                : await CaptureReservedFrozenAsync(request, freezeTarget, token);
        }
        catch (OperationCanceledException)
        {
            return new CaptureAcquireResult(Cancelled("Capture was cancelled by the caller."), null);
        }
        catch (Exception exception)
        {
            DiagnosticContext.CaptureFailure(
                stage: freezeTarget is null ? "DisplayCapture" : "RegionCapture",
                userFacingState: "Capture failed",
                technicalDetail: exception.Message,
                correlationId: request.CorrelationId,
                exception: exception).LogTo(LoggerHolder.Value);
            return new CaptureAcquireResult(
                new WindowsCaptureResult(
                    WindowsCaptureOutcome.Failed,
                    "Capture failed",
                    $"{exception.GetType().Name}: {exception.Message}"),
                null);
        }
        finally
        {
            updateSessionState(CaptureSessionState.Idle());
            operationGate.Release();
        }
    }

    private async Task<CaptureAcquireResult> CaptureReservedDisplayAsync(
        WindowsCaptureRequest request,
        CancellationToken cancellationToken)
    {
        var selection = await selectTargetAsync(cancellationToken);
        updateSessionState(CaptureSessionState.FromSelectionResult(selection));
        if (!selection.IsSelected)
        {
            var outcome = selection.IsUnsupported
                ? WindowsCaptureOutcome.Unsupported
                : selection.IsCanceled
                    ? WindowsCaptureOutcome.Cancelled
                    : WindowsCaptureOutcome.Unavailable;
            return new CaptureAcquireResult(
                new WindowsCaptureResult(
                    outcome,
                    selection.Readiness.UserMessage,
                    selection.Readiness.TechnicalDetail ?? selection.Readiness.UserMessage),
                null);
        }

        return await CaptureSelectedTargetAsync(
            request,
            selection.Target,
            capability: null,
            holdFrame: false,
            cancellationToken);
    }

    private async Task<CaptureAcquireResult> CaptureReservedFrozenAsync(
        WindowsCaptureRequest request,
        WindowsTargetCapability targetSnapshot,
        CancellationToken cancellationToken)
    {
        try
        {
            cancellationToken.ThrowIfCancellationRequested();
            var target = targetSnapshot.CreateCaptureTarget();
            updateSessionState(CaptureSessionState.FromSelectionResult(
                CaptureTargetSelectionResult.Selected(
                    target,
                    EngineReadinessStatus.Initializing(
                        EngineReadinessStage.Capture,
                        "Region target selected.",
                        $"Resolved issued target {target.DisplayName}."))));
            return await CaptureSelectedTargetAsync(
                request,
                target,
                targetSnapshot,
                holdFrame: true,
                cancellationToken);
        }
        catch (ArgumentOutOfRangeException exception)
        {
            return new CaptureAcquireResult(
                new WindowsCaptureResult(
                    WindowsCaptureOutcome.Unavailable,
                    "Region capture is unavailable",
                    exception.Message),
                null);
        }
        catch (InvalidOperationException exception)
        {
            return new CaptureAcquireResult(
                new WindowsCaptureResult(
                    WindowsCaptureOutcome.Unavailable,
                    "Region capture is unavailable",
                    exception.Message),
                null);
        }
    }

    private async Task<CaptureAcquireResult> CaptureSelectedTargetAsync(
        WindowsCaptureRequest request,
        CaptureTarget target,
        WindowsTargetCapability? capability,
        bool holdFrame,
        CancellationToken cancellationToken)
    {
        var hdrCapability = probeHdrCapability(target);
        var frameCompletion = new TaskCompletionSource<FrameCaptureResult>(
            TaskCreationOptions.RunContinuationsAsynchronously);
        var acceptingFrames = 1;

        void ReceiveFrame(CapturedFrameTexture frame)
        {
            if (Volatile.Read(ref acceptingFrames) == 0
                || !frameCompletion.TrySetResult(FrameCaptureResult.Succeeded(frame)))
            {
                frame.Dispose();
            }
        }

        void ReceiveFailure(EngineReadinessStatus readiness) =>
            frameCompletion.TrySetResult(FrameCaptureResult.Failed(readiness));

        using var cancellationRegistration = cancellationToken.Register(() =>
        {
            Volatile.Write(ref acceptingFrames, 0);
            frameCompletion.TrySetCanceled(cancellationToken);
        });

        var startResult = startCapture(target, ReceiveFrame, ReceiveFailure);
        updateSessionState(CaptureSessionState.FromStartResult(target, startResult));
        if (!startResult.Started)
        {
            return new CaptureAcquireResult(
                new WindowsCaptureResult(
                    startResult.Readiness.State == EngineReadinessState.Unsupported
                        ? WindowsCaptureOutcome.Unsupported
                        : WindowsCaptureOutcome.Failed,
                    startResult.Readiness.UserMessage,
                    startResult.Readiness.TechnicalDetail ?? startResult.Readiness.UserMessage,
                    hdrCapability),
                null);
        }

        using var sessionResources = startResult.SessionResources!;
        FrameCaptureResult frameResult;
        try
        {
            frameResult = await frameCompletion.Task.WaitAsync(frameTimeout, cancellationToken);
        }
        catch (OperationCanceledException)
        {
            Volatile.Write(ref acceptingFrames, 0);
            if (!frameCompletion.TrySetCanceled(cancellationToken)
                && frameCompletion.Task.IsCompletedSuccessfully)
            {
                frameCompletion.Task.Result.Texture?.Dispose();
            }

            throw;
        }
        catch (TimeoutException)
        {
            Volatile.Write(ref acceptingFrames, 0);
            if (!frameCompletion.TrySetCanceled()
                && frameCompletion.Task.IsCompletedSuccessfully)
            {
                frameCompletion.Task.Result.Texture?.Dispose();
            }

            return new CaptureAcquireResult(
                new WindowsCaptureResult(
                    WindowsCaptureOutcome.TimedOut,
                    "Capture timed out",
                    $"No frame arrived within {frameTimeout}.",
                    hdrCapability),
                null);
        }
        finally
        {
            Volatile.Write(ref acceptingFrames, 0);
        }

        if (frameResult.Texture is null)
        {
            var readiness = frameResult.Readiness!;
            return new CaptureAcquireResult(
                new WindowsCaptureResult(
                    WindowsCaptureOutcome.Failed,
                    readiness.UserMessage,
                    readiness.TechnicalDetail ?? readiness.UserMessage,
                    hdrCapability),
                null);
        }

        var source = frameResult.Texture;
        CapturedFrameTexture owned;
        try
        {
            owned = holdFrame ? copyOwnedFrame(source) : source;
            if (holdFrame && !ReferenceEquals(owned, source))
            {
                source.Dispose();
            }
        }
        catch
        {
            source.Dispose();
            throw;
        }

        updateSessionState(CaptureSessionState.Capturing(
            target,
            EngineReadinessStatus.Ready(
                "Captured frame is ready for sRGB Visual Match conversion.",
                $"Captured {owned.Width}x{owned.Height} from {target.DisplayName}.")));

        if (holdFrame)
        {
            return new CaptureAcquireResult(
                new WindowsCaptureResult(
                    WindowsCaptureOutcome.Delivered,
                    "Frozen region frame is ready.",
                    $"Captured {owned.Width}x{owned.Height} from {target.DisplayName}.",
                    hdrCapability),
                new HeldFrozenFrame(owned, target, capability!, hdrCapability));
        }

        try
        {
            return new CaptureAcquireResult(
                await DeliverHeldFrameAsync(
                    request,
                    owned,
                    hdrCapability,
                    cropRegion: null,
                    cancellationToken),
                null);
        }
        finally
        {
            owned.Dispose();
        }
    }

    private async Task<WindowsCaptureResult> DeliverHeldFrameAsync(
        WindowsCaptureRequest request,
        CapturedFrameTexture texture,
        HdrDisplayCapability hdrCapability,
        CropPixelRect? cropRegion,
        CancellationToken cancellationToken)
    {
        var outputResult = await output.ExecuteOutputAsync(
            new OutputRequest
            {
                Texture = texture,
                CropRegion = cropRegion,
                VisualMatchContext = ResolveVisualMatchContext(hdrCapability),
                Delivery = request.Delivery,
                SaveDirectory = request.SaveDirectory,
                TimestampNaming = request.TimestampNaming,
            },
            cancellationToken);

        return new WindowsCaptureResult(
            outputResult.IsSuccess
                ? WindowsCaptureOutcome.Delivered
                : WindowsCaptureOutcome.DeliveryFailed,
            outputResult.UserMessage,
            outputResult.TechnicalDetail,
            hdrCapability,
            outputResult);
    }

    private static SrgbVisualMatchConversionContext ResolveVisualMatchContext(
        HdrDisplayCapability hdrCapability)
    {
        if (!hdrCapability.IsHdrActive)
        {
            return SrgbVisualMatchConversionContext.ForSdrDisplay();
        }

        if (hdrCapability.SdrWhiteLevelInNits is not { } sdrWhiteLevelInNits)
        {
            throw new OutputArtifactEncodingException(
                "The HDR target's SDR white level is unavailable; sRGB Visual Match conversion cannot be validated.");
        }

        return SrgbVisualMatchConversionContext.ForHdrDisplay(sdrWhiteLevelInNits);
    }

    private static WindowsCaptureResult Cancelled(string detail) =>
        new(WindowsCaptureOutcome.Cancelled, "Capture cancelled", detail);

    private bool HasFrozenRegion()
    {
        lock (frozenLock)
        {
            return frozenRegion is not null;
        }
    }

    private async Task ReleaseFrozenRegionAsync(string? sessionId = null)
    {
        FrozenRegionSession? session;
        lock (frozenLock)
        {
            if (frozenRegion is null || (sessionId is not null && frozenRegion.Id != sessionId))
            {
                return;
            }

            session = frozenRegion;
            frozenRegion = null;
        }

        session.Lease.Dispose();
        session.Frame.Dispose();
        TryDeletePreview(session.PreviewPath);
        await Task.CompletedTask;
    }

    private static string WriteRegionPreview(byte[] previewBytes)
    {
        var directory = Path.Combine(Path.GetTempPath(), "lumiere-region-preview");
        Directory.CreateDirectory(directory);
        var path = Path.Combine(directory, $"{Guid.NewGuid():N}.png");
        File.WriteAllBytes(path, previewBytes);
        return path;
    }

    private static void TryDeletePreview(string path)
    {
        try
        {
            if (File.Exists(path))
            {
                File.Delete(path);
            }
        }
        catch (IOException)
        {
        }
        catch (UnauthorizedAccessException)
        {
        }
    }

    private void ThrowIfDisposed()
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref disposed) != 0, this);
    }

    public async ValueTask DisposeAsync()
    {
        if (Interlocked.Exchange(ref disposed, 1) != 0)
        {
            return;
        }

        await lifetimeCancellation.CancelAsync();
        await nativeRegionGate.WaitAsync();
        await operationGate.WaitAsync();
        try
        {
            await ReleaseFrozenRegionAsync();
            (nativeRegionOverlay as IDisposable)?.Dispose();
            nativeRegionRenderer?.Dispose();
            ownedResources?.Dispose();
        }
        finally
        {
            operationGate.Release();
            operationGate.Dispose();
            nativeRegionGate.Release();
            nativeRegionGate.Dispose();
            lifetimeCancellation.Dispose();
        }
    }

    private sealed record CaptureAcquireResult(
        WindowsCaptureResult Result,
        HeldFrozenFrame? HeldFrame);

    private sealed class HeldFrozenFrame(
        CapturedFrameTexture texture,
        CaptureTarget captureTarget,
        WindowsTargetCapability target,
        HdrDisplayCapability hdrCapability) : IDisposable
    {
        public CapturedFrameTexture Texture { get; } = texture;
        public CaptureTarget CaptureTarget { get; } = captureTarget;
        public WindowsTargetCapability Target { get; } = target;
        public HdrDisplayCapability HdrCapability { get; } = hdrCapability;

        public void Dispose() => Texture.Dispose();
    }

    private sealed class FrozenRegionSession(
        string id,
        HeldFrozenFrame frame,
        string previewPath,
        CancellationTokenSource lease)
    {
        public string Id { get; } = id;
        public HeldFrozenFrame Frame { get; } = frame;
        public string PreviewPath { get; } = previewPath;
        public CancellationTokenSource Lease { get; } = lease;
    }

    private sealed record FrameCaptureResult(
        CapturedFrameTexture? Texture,
        EngineReadinessStatus? Readiness)
    {
        public static FrameCaptureResult Succeeded(CapturedFrameTexture texture) =>
            new(texture, null);

        public static FrameCaptureResult Failed(EngineReadinessStatus readiness) =>
            new(null, readiness);
    }
}
