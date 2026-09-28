using System.Threading;
using Lumiere.Windows.Graphics.Devices;
using Lumiere.Windows.Graphics.Hdr;
using Lumiere.Windows.Graphics.Presentation;
using Lumiere.Windows.Interop;
using Lumiere.Windows.Interop.Diagnostics;
using Microsoft.Extensions.Logging;
using Vortice.Direct3D11;
using Vortice.DXGI;
using Vortice.Mathematics;
using Windows.Foundation;
using Windows.Graphics.Capture;
using Windows.Graphics.DirectX.Direct3D11;

namespace Lumiere.Windows.Capture;

internal sealed class CaptureService
{
    private static readonly ILogger Logger = LumiereLoggerFactory.CreateLogger(LogCategories.Capture);
    private readonly GraphicsDeviceResources deviceResources;
    private readonly CaptureBorderOptions borderOptions;
    private readonly object commandLock = new();
    private CaptureSessionState sessionState = CaptureSessionState.Idle();

    public CaptureService(
        GraphicsDeviceResources deviceResources,
        CaptureBorderOptions? borderOptions = null)
    {
        this.deviceResources = deviceResources ?? throw new ArgumentNullException(nameof(deviceResources));
        this.borderOptions = borderOptions ?? CaptureBorderOptions.RequireSystemBorder();
    }

    /// <summary>
    /// Gets the current capture session state. Thread-safe read.
    /// </summary>
    public CaptureSessionState CurrentSessionState
    {
        get { lock (commandLock) { return sessionState; } }
    }

    /// <summary>
    /// Updates the session state. Thread-safe write.
    /// </summary>
    /// <param name="newState">The new session state.</param>
    public void UpdateSessionState(CaptureSessionState newState)
    {
        ArgumentNullException.ThrowIfNull(newState);
        lock (commandLock)
        {
            sessionState = newState;
        }
    }

    public CaptureTarget CreateTarget(GraphicsCaptureItem item) =>
        CaptureTarget.FromItem(item);

    /// <summary>
    /// Checks whether the current session state allows accepting a new capture command.
    /// </summary>
    /// <param name="currentState">The current capture session state.</param>
    /// <param name="command">The command to validate.</param>
    /// <param name="rejectionReason">If rejected, contains the readiness status explaining why.</param>
    /// <returns>True if the command can be accepted; false otherwise.</returns>
    public static bool CanAcceptCommand(
        CaptureSessionState currentState,
        CaptureCommand command,
        out EngineReadinessStatus? rejectionReason)
    {
        ArgumentNullException.ThrowIfNull(currentState);
        ArgumentNullException.ThrowIfNull(command);

        // Accept commands only when session is Idle or Failed (recoverable)
        switch (currentState.Status)
        {
            case CaptureSessionStatus.Idle:
                rejectionReason = null;
                return true;

            case CaptureSessionStatus.Failed:
            case CaptureSessionStatus.Unsupported:
                // Failed/Unsupported states are recoverable - allow retry
                rejectionReason = null;
                return true;

            case CaptureSessionStatus.SelectingTarget:
            case CaptureSessionStatus.Initializing:
            case CaptureSessionStatus.Capturing:
            case CaptureSessionStatus.Degraded:
            case CaptureSessionStatus.Disposed:
            default:
                rejectionReason = EngineReadinessStatus.Initializing(
                    EngineReadinessStage.Capture,
                    "Capture already active",
                    $"Cannot accept {command.Mode} command while session is {currentState.Status}.");
                return false;
        }
    }

    /// <summary>
    /// Classifies a rejected command into the appropriate rejection outcome.
    /// Single authoritative mapping from session status to rejection classification.
    /// </summary>
    private static CaptureCommandResult ClassifyRejection(
        CaptureCommand command,
        CaptureSessionState sessionState,
        EngineReadinessStatus? rejectionReason)
    {
        return sessionState.Status is CaptureSessionStatus.SelectingTarget
            or CaptureSessionStatus.Initializing
            or CaptureSessionStatus.Capturing
            or CaptureSessionStatus.Degraded
            ? CaptureCommandResult.RejectedSessionActive(command, sessionState, rejectionReason)
            : CaptureCommandResult.RejectedNonRecoverable(command, sessionState, rejectionReason);
    }

    /// <summary>
    /// Validates whether a capture command can be accepted by the current session state.
    /// This is the primary entry point for capture commands from any app-facing entry point.
    /// Thread-safe: the guard check is atomic with state read.
    /// </summary>
    /// <param name="command">The capture command to validate.</param>
    /// <returns>A CaptureCommandResult indicating acceptance or rejection.</returns>
    public CaptureCommandResult ValidateCommand(CaptureCommand command)
    {
        ArgumentNullException.ThrowIfNull(command);

        lock (commandLock)
        {
            Logger.LogInformation(
                "ValidateCommand: mode={Mode}, currentStatus={Status}",
                command.Mode, sessionState.Status);

            if (!CanAcceptCommand(sessionState, command, out var rejectionReason))
            {
                var diagnostic = DiagnosticContext.CaptureWarning(
                    stage: "CommandGuard",
                    userFacingState: "Capture command rejected",
                    technicalDetail: $"mode={command.Mode}, currentStatus={sessionState.Status}, reason={rejectionReason?.TechnicalDetail ?? "none"}");
                diagnostic.LogTo(Logger);

                return ClassifyRejection(command, sessionState, rejectionReason);
            }

            Logger.LogInformation(
                "ValidateCommand ACCEPTED: mode={Mode}, target={TargetDisplayName}",
                command.Mode, command.Target?.DisplayName ?? "(deferred)");
            return CaptureCommandResult.Accepted(command);
        }
    }

    /// <summary>
    /// Atomically validates and reserves a capture command by transitioning session state
    /// to SelectingTarget. This eliminates the TOCTOU gap between the command guard and state transition.
    /// Thread-safe: guard check and state write happen under the same lock.
    /// </summary>
    /// <param name="command">The capture command to validate and reserve.</param>
    /// <returns>A CaptureCommandResult indicating acceptance or rejection.</returns>
    public CaptureCommandResult TryReserveCommand(CaptureCommand command)
    {
        ArgumentNullException.ThrowIfNull(command);

        lock (commandLock)
        {
            Logger.LogInformation(
                "TryReserveCommand: mode={Mode}, currentStatus={Status}",
                command.Mode, sessionState.Status);

            if (!CanAcceptCommand(sessionState, command, out var rejectionReason))
            {
                var diagnostic = DiagnosticContext.CaptureWarning(
                    stage: "CommandReservation",
                    userFacingState: "Capture command rejected",
                    technicalDetail: $"mode={command.Mode}, currentStatus={sessionState.Status}, reason={rejectionReason?.TechnicalDetail ?? "none"}");
                diagnostic.LogTo(Logger);

                return ClassifyRejection(command, sessionState, rejectionReason);
            }

            sessionState = CaptureSessionState.SelectingTarget(
                EngineReadinessStatus.Initializing(
                    EngineReadinessStage.Capture,
                    "Starting capture...",
                    "Command reserved, preparing capture."));

            Logger.LogInformation(
                "TryReserveCommand ACCEPTED: mode={Mode}, newStatus=SelectingTarget",
                command.Mode);
            return CaptureCommandResult.Accepted(command);
        }
    }

    internal CapturedFrameTexture CopyToOwnedTexture(CapturedFrameTexture source)
    {
        ArgumentNullException.ThrowIfNull(source);
        if (source.Texture is null)
        {
            return source;
        }

        var description = source.Texture.Description;
        description.Width = (uint)source.Width;
        description.Height = (uint)source.Height;
        description.MipLevels = 1;
        description.ArraySize = 1;
        description.Usage = ResourceUsage.Default;
        description.BindFlags = BindFlags.ShaderResource;
        description.CPUAccessFlags = CpuAccessFlags.None;
        description.MiscFlags = ResourceOptionFlags.None;
        description.SampleDescription = new SampleDescription(1, 0);

        var owned = deviceResources.Device.CreateTexture2D(description);
        var sourceBox = new Box(0, 0, 0, source.Width, source.Height, 1);
        lock (deviceResources.ImmediateContextSync)
        {
            deviceResources.ImmediateContext.CopySubresourceRegion(
                owned,
                0,
                0,
                0,
                0,
                source.Texture,
                0,
                sourceBox);
        }
        return new CapturedFrameTexture(owned, source.Width, source.Height, "Owned frozen frame");
    }

    public CaptureStartResult StartCapture(
        CaptureTarget target,
        Action<CapturedFrameTexture> onFrameArrived,
        Action<EngineReadinessStatus>? onFrameFailed = null)
    {
        ArgumentNullException.ThrowIfNull(target);
        ArgumentNullException.ThrowIfNull(onFrameArrived);

        if (!target.HasCaptureItem)
        {
            Logger.LogWarning("StartCapture FAILED: CaptureTarget has no GraphicsCaptureItem");
            return CaptureStartResult.NotStarted(
                EngineReadinessStatus.Failed(
                    EngineReadinessStage.Capture,
                    "Capture failed",
                    "CaptureTarget was created for tests and does not contain a GraphicsCaptureItem."));
        }

        if (!GraphicsCaptureSession.IsSupported())
        {
            Logger.LogWarning("StartCapture FAILED: GraphicsCaptureSession.IsSupported=false");
            return CaptureStartResult.NotStarted(
                EngineReadinessStatus.Unsupported(
                    EngineReadinessStage.Capture,
                    "Unsupported capture",
                    "GraphicsCaptureSession.IsSupported returned false."));
        }

        IDirect3DDevice? direct3DDevice = null;
        Direct3D11CaptureFramePool? framePool = null;
        GraphicsCaptureSession? session = null;
        TypedEventHandler<Direct3D11CaptureFramePool, object?>? frameArrivedHandler = null;
        var frameFailureGate = new FrameFailureGate();

        try
        {
            var options = new CaptureSessionOptions(target.Size.Width, target.Size.Height);
            direct3DDevice = Direct3D11Interop.CreateDirect3DDevice(deviceResources.DxgiDevice);
            framePool = Direct3D11CaptureFramePool.CreateFreeThreaded(
                direct3DDevice,
                options.PixelFormat,
                options.BufferCount,
                options.BufferSize);

            Logger.LogDebug("WGC FramePool created: pixelFormat={PixelFormat}, bufferSize={BufferSize}, target={DisplayName} ({Width}x{Height})", options.PixelFormat, options.BufferCount, target.DisplayName, target.Size.Width, target.Size.Height);

            frameArrivedHandler = (sender, _) =>
            {
                if (!frameFailureGate.ShouldProcessFrame)
                {
                    return;
                }

                HandleFrameArrived(
                    sender,
                    onFrameArrived,
                    onFrameFailed,
                    frameFailureGate);
            };
            framePool.FrameArrived += frameArrivedHandler;
            session = framePool.CreateCaptureSession(target.Item);
            var borderResult = borderOptions.ApplyToSession(session);
            LogBorderResult(borderResult);
            session.StartCapture();

            Logger.LogInformation("WGC session started: IsSupported=true, target={DisplayName} ({Width}x{Height}), kind={Kind}", target.DisplayName, target.Size.Width, target.Size.Height, target.Kind);

            return CaptureStartResult.StartSucceeded(
                new CaptureSessionResources(direct3DDevice, framePool, session, frameArrivedHandler),
                EngineReadinessStatus.Initializing(
                    EngineReadinessStage.Capture,
                    "Initializing capture",
                    $"Direct3D11CaptureFramePool started with {options.PixelFormat} and {options.BufferCount} buffers for {target.DisplayName}. {borderResult.TechnicalDetail}"));
        }
        catch (Exception exception)
        {
            _ = CaptureSessionDisposalCoordinator.DisposeOnce(
                () =>
                {
                    if (framePool is not null && frameArrivedHandler is not null)
                    {
                        framePool.FrameArrived -= frameArrivedHandler;
                    }
                },
                () => (session as IDisposable)?.Dispose(),
                () => (framePool as IDisposable)?.Dispose(),
                () => (direct3DDevice as IDisposable)?.Dispose());

            var diagnostic = DiagnosticContext.CaptureFailure(
                stage: "StartCapture",
                userFacingState: "Capture failed",
                technicalDetail: exception is NativeInteropException nativeEx
                    ? $"Operation={nativeEx.OperationName}, Stage={nativeEx.Stage}, HRESULT={NativeInteropException.FormatHResult(nativeEx.HResultCode)}, Detail={nativeEx.TechnicalDetail}"
                    : $"Exception={exception.GetType().Name}: {exception.Message}",
                exception: exception);
            diagnostic.LogTo(Logger);

            return CaptureStartResult.NotStarted(MapFailureToReadiness(exception));
        }
    }

    private static void LogBorderResult(CaptureBorderApplicationResult borderResult)
    {
        if (borderResult.RequestedBorderless && !borderResult.Succeeded)
        {
            Logger.LogWarning(
                "WGC borderless request did not take effect: attempted={Attempted}, effectiveIsBorderRequired={EffectiveIsBorderRequired}, detail={Detail}",
                borderResult.Attempted,
                borderResult.EffectiveIsBorderRequired,
                borderResult.TechnicalDetail);
            return;
        }

        Logger.LogDebug(
            "WGC border policy applied: requestedBorderless={RequestedBorderless}, attempted={Attempted}, effectiveIsBorderRequired={EffectiveIsBorderRequired}, detail={Detail}",
            borderResult.RequestedBorderless,
            borderResult.Attempted,
            borderResult.EffectiveIsBorderRequired,
            borderResult.TechnicalDetail);
    }

    public static EngineReadinessStatus MapFailureToReadiness(Exception exception)
    {
        ArgumentNullException.ThrowIfNull(exception);

        if (exception is NativeInteropException nativeInteropException)
        {
            return EngineReadinessStatus.Failed(
                EngineReadinessStage.Interop,
                "Capture failed",
                $"Operation={nativeInteropException.OperationName}, Stage={nativeInteropException.Stage}, HRESULT={NativeInteropException.FormatHResult(nativeInteropException.HResultCode)}, Detail={nativeInteropException.TechnicalDetail}");
        }

        return EngineReadinessStatus.Failed(
            EngineReadinessStage.Capture,
            "Capture failed",
            $"Exception={exception.GetType().Name}: {exception.Message}");
    }

    private static void HandleFrameArrived(
        Direct3D11CaptureFramePool framePool,
        Action<CapturedFrameTexture> onFrameArrived,
        Action<EngineReadinessStatus>? onFrameFailed,
        FrameFailureGate frameFailureGate)
    {
        try
        {
            using var frame = framePool.TryGetNextFrame();
            if (frame is null)
            {
                return;
            }

            CapturedFrameTexture? capturedFrame = null;

            try
            {
                capturedFrame = new CapturedFrameTexture(
                    Direct3D11SurfaceInterop.CreateTexture(frame.Surface),
                    frame.ContentSize.Width,
                    frame.ContentSize.Height,
                    "Direct3D11CaptureFrame.Surface");
                onFrameArrived(capturedFrame);
                capturedFrame = null;
            }
            finally
            {
                capturedFrame?.Dispose();
            }
        }
        catch (Exception exception)
        {
            if (frameFailureGate.TryMarkFailed())
            {
                var diagnostic = DiagnosticContext.CaptureFailure(
                    stage: "FrameArrived",
                    userFacingState: "Capture failed",
                    technicalDetail: $"Frame processing failed: {exception.GetType().Name}: {exception.Message}",
                    exception: exception);
                diagnostic.LogTo(Logger);
                TryReportFrameFailure(exception, onFrameFailed);
            }
        }
    }

    private static void TryReportFrameFailure(
        Exception exception,
        Action<EngineReadinessStatus>? onFrameFailed)
    {
        try
        {
            onFrameFailed?.Invoke(MapFailureToReadiness(exception));
        }
        catch
        {
            // FrameArrived runs outside the UI thread; teardown races must not escape the WGC callback.
        }
    }

    private sealed class FrameFailureGate
    {
        private int failed;

        public bool ShouldProcessFrame => Volatile.Read(ref failed) == 0;

        public bool TryMarkFailed() => Interlocked.Exchange(ref failed, 1) == 0;
    }
}
