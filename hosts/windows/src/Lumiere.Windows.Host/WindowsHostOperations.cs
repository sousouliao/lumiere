using Lumiere.Windows.Capture;
using Lumiere.Windows.Graphics.Output;
using Microsoft.Extensions.Logging;
using Microsoft.Extensions.Logging.Abstractions;

namespace Lumiere.Windows.Host;

public interface IWindowsCaptureEngine : IAsyncDisposable
{
    Task<WindowsCaptureResult> CaptureDisplayAsync(
        WindowsCaptureRequest request,
        CancellationToken cancellationToken = default);

    Task<WindowsPrepareRegionResult> PrepareRegionAsync(
        string requestId,
        WindowsTargetCapability target,
        CancellationToken cancellationToken = default);

    Task<WindowsCaptureResult> CommitRegionAsync(
        string sessionId,
        WindowsCaptureRequest request,
        WindowsRegionGeometry geometry,
        CancellationToken cancellationToken = default);

    Task ReleaseRegionAsync(string sessionId);
}

public sealed class WindowsHostOperations : IWindowsHostOperations
{
    private const int IssuedTargetLimit = 8;
    private readonly object sync = new();
    private readonly Func<CancellationToken, Task<IWindowsCaptureEngine>> engineFactory;
    private readonly Func<WindowsTargetCapability?> getTargetCapability;
    private readonly Func<string> targetTokenFactory;
    private readonly Func<string> outputDirectory;
    private readonly Action<string> createDirectory;
    private readonly ILogger logger;
    private readonly SemaphoreSlim engineCreationGate = new(1, 1);
    private IWindowsCaptureEngine? engine;
    private readonly List<IssuedTarget> issuedTargets = [];
    private int disposed;

    public WindowsHostOperations(
        Func<IWindowsCaptureEngine> engineFactory,
        Func<WindowsTargetCapability?> getTargetCapability,
        Func<string> outputDirectory,
        Action<string> createDirectory,
        ILogger? logger = null,
        Func<string>? targetTokenFactory = null)
    {
        ArgumentNullException.ThrowIfNull(engineFactory);
        this.engineFactory = _ => Task.FromResult(engineFactory());
        this.getTargetCapability = getTargetCapability
            ?? throw new ArgumentNullException(nameof(getTargetCapability));
        this.targetTokenFactory = targetTokenFactory ?? (() => Guid.NewGuid().ToString("N"));
        this.outputDirectory = outputDirectory ?? throw new ArgumentNullException(nameof(outputDirectory));
        this.createDirectory = createDirectory ?? throw new ArgumentNullException(nameof(createDirectory));
        this.logger = logger ?? NullLogger.Instance;
    }

    public static WindowsHostOperations CreateDefault(ILogger? logger = null)
    {
        var capabilityProvider = new WindowsTargetCapabilityProvider();
        return new WindowsHostOperations(
            static async cancellationToken => new WindowsDisplayCaptureEngineAdapter(
                await WindowsDisplayCaptureEngine.CreateDefaultAsync(cancellationToken)),
            capabilityProvider.GetCurrent,
            WindowsCaptureFolder.GetDefaultPath,
            static path => _ = Directory.CreateDirectory(path),
            logger,
            static () => Guid.NewGuid().ToString("N"));
    }

    private WindowsHostOperations(
        Func<CancellationToken, Task<IWindowsCaptureEngine>> engineFactory,
        Func<WindowsTargetCapability?> getTargetCapability,
        Func<string> outputDirectory,
        Action<string> createDirectory,
        ILogger? logger,
        Func<string>? targetTokenFactory = null)
    {
        this.engineFactory = engineFactory ?? throw new ArgumentNullException(nameof(engineFactory));
        this.getTargetCapability = getTargetCapability
            ?? throw new ArgumentNullException(nameof(getTargetCapability));
        this.targetTokenFactory = targetTokenFactory ?? (() => Guid.NewGuid().ToString("N"));
        this.outputDirectory = outputDirectory ?? throw new ArgumentNullException(nameof(outputDirectory));
        this.createDirectory = createDirectory ?? throw new ArgumentNullException(nameof(createDirectory));
        this.logger = logger ?? NullLogger.Instance;
    }

    public HostCapabilities GetCapabilities()
    {
        ObjectDisposedException.ThrowIf(Volatile.Read(ref disposed) != 0, this);
        var target = getTargetCapability();
        HostCaptureTarget? activeTarget = null;
        if (target?.SupportsRegionCapture == true && target.LogicalSize is { } logicalSize)
        {
            var token = targetTokenFactory();
            if (!string.IsNullOrWhiteSpace(token))
            {
                activeTarget = new HostCaptureTarget(
                    token,
                    new HostLogicalSize(logicalSize.Width, logicalSize.Height));
            }
        }
        lock (sync)
        {
            if (activeTarget is not null)
            {
                issuedTargets.Add(new IssuedTarget(activeTarget.Id, target!));
                if (issuedTargets.Count > IssuedTargetLimit)
                {
                    issuedTargets.RemoveAt(0);
                }
            }
        }
        var supportsRegion = activeTarget is not null;
        return new HostCapabilities(
            PlatformProtocol.ContractVersion,
            "windows",
            "available",
            supportsRegion ? ["region", "display"] : ["display"],
            ["clipboard", "folder"],
            MapHdrCapture(target?.HdrState),
            ["srgb-visual-match"],
            activeTarget);
    }

    public Task<HostCaptureResult> CaptureDisplayAsync(
        string requestId,
        HostCaptureRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(requestId);
        ArgumentNullException.ThrowIfNull(request);
        ObjectDisposedException.ThrowIf(Volatile.Read(ref disposed) != 0, this);
        return ExecuteCaptureAsync(
            requestId,
            request.Delivery,
            request.SaveDirectory,
            captureMode: "display",
            (engine, captureRequest, token) => engine.CaptureDisplayAsync(captureRequest, token),
            cancellationToken);
    }

    public async Task<HostPrepareRegionResult> PrepareRegionAsync(
        string requestId,
        string targetId,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(requestId);
        ArgumentException.ThrowIfNullOrWhiteSpace(targetId);
        ObjectDisposedException.ThrowIf(Volatile.Read(ref disposed) != 0, this);

        WindowsTargetCapability? target = null;
        lock (sync)
        {
            var targetIndex = issuedTargets.FindIndex(candidate => candidate.Token == targetId);
            if (targetIndex >= 0)
            {
                target = issuedTargets[targetIndex].Target;
                issuedTargets.RemoveAt(targetIndex);
            }
        }
        if (target?.SupportsRegionCapture != true || target.LogicalSize is null)
        {
            return PrepareFailed(
                "capture-unavailable",
                "Region capture is unavailable on the current target.",
                retryable: true);
        }

        try
        {
            var captureEngine = await GetOrCreateEngineAsync(cancellationToken);
            var prepared = await captureEngine.PrepareRegionAsync(requestId, target, cancellationToken);
            if (!prepared.Prepared
                || string.IsNullOrWhiteSpace(prepared.SessionId)
                || string.IsNullOrWhiteSpace(prepared.PreviewPath)
                || prepared.TargetLogicalSize is null
                || prepared.PreviewPixelWidth <= 0
                || prepared.PreviewPixelHeight <= 0)
            {
                return MapPrepareFailure(prepared, requestId);
            }

            return new HostPrepareRegionResult(
                "prepared",
                prepared.SessionId,
                new HostLogicalSize(
                    prepared.TargetLogicalSize.Width,
                    prepared.TargetLogicalSize.Height),
                new HostRegionPreview(
                    prepared.PreviewPath,
                    "image/png",
                    new HostPixelSize(prepared.PreviewPixelWidth, prepared.PreviewPixelHeight)),
                prepared.LeaseMilliseconds);
        }
        catch (OperationCanceledException)
        {
            return PrepareFailed(
                "capture-unavailable",
                "Windows capture timed out. Try again.",
                retryable: true);
        }
        catch (Exception exception)
        {
            logger.LogError(
                exception,
                "operation=PrepareRegion, stage=Failed, correlation={CorrelationId}",
                requestId);
            return PrepareFailed(
                "unexpected-failure",
                "Windows capture failed. Try again.",
                retryable: true);
        }
    }

    public Task<HostCaptureResult> CommitRegionAsync(
        string requestId,
        HostCommitRegionRequest request,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(requestId);
        ArgumentNullException.ThrowIfNull(request);
        ObjectDisposedException.ThrowIf(Volatile.Read(ref disposed) != 0, this);

        var geometry = new WindowsRegionGeometry(
            request.Geometry.X,
            request.Geometry.Y,
            request.Geometry.Width,
            request.Geometry.Height);
        return ExecuteCaptureAsync(
            requestId,
            request.Delivery,
            request.SaveDirectory,
            captureMode: "region",
            (engine, captureRequest, token) => engine.CommitRegionAsync(
                request.SessionId,
                captureRequest,
                geometry,
                token),
            cancellationToken);
    }

    public async Task<HostReleasedRegion> CancelRegionAsync(string sessionId)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(sessionId);
        ObjectDisposedException.ThrowIf(Volatile.Read(ref disposed) != 0, this);
        IWindowsCaptureEngine? ownedEngine;
        lock (sync)
        {
            ownedEngine = engine;
        }

        if (ownedEngine is not null)
        {
            await ownedEngine.ReleaseRegionAsync(sessionId);
        }

        return new HostReleasedRegion();
    }

    private async Task<HostCaptureResult> ExecuteCaptureAsync(
        string requestId,
        string deliveryValue,
        string? saveDirectory,
        string captureMode,
        Func<IWindowsCaptureEngine, WindowsCaptureRequest, CancellationToken, Task<WindowsCaptureResult>> capture,
        CancellationToken cancellationToken)
    {
        if (!TryMapDelivery(deliveryValue, out var delivery))
        {
            return Failed(
                "delivery-unavailable",
                "Windows capture supports clipboard, folder, or both delivery.",
                retryable: false);
        }

        try
        {
            string? directory = null;
            if (delivery is OutputTarget.Folder or OutputTarget.Both)
            {
                directory = saveDirectory ?? outputDirectory();
                if (string.IsNullOrWhiteSpace(directory))
                {
                    throw new InvalidOperationException("The Windows capture folder could not be resolved.");
                }

                try
                {
                    createDirectory(directory);
                }
                catch (Exception exception)
                {
                    logger.LogWarning(
                        exception,
                        "operation=CaptureDirectory, stage=CreateFailed, correlation={CorrelationId}",
                        requestId);
                }
            }

            var captureRequest = new WindowsCaptureRequest(requestId, delivery, directory);
            var captureEngine = await GetOrCreateEngineAsync(cancellationToken);
            var captureResult = await capture(captureEngine, captureRequest, cancellationToken);
            return MapCaptureResult(captureResult, delivery, requestId, captureMode);
        }
        catch (OperationCanceledException)
        {
            return new HostCaptureResult("cancelled");
        }
        catch (Exception exception)
        {
            logger.LogError(
                exception,
                "operation=CaptureRequest, stage=Failed, correlation={CorrelationId}",
                requestId);
            return Failed(
                "unexpected-failure",
                "Windows capture failed. Try again.",
                retryable: true);
        }
    }

    private async Task<IWindowsCaptureEngine> GetOrCreateEngineAsync(
        CancellationToken cancellationToken)
    {
        await engineCreationGate.WaitAsync(cancellationToken);
        try
        {
            lock (sync)
            {
                ObjectDisposedException.ThrowIf(disposed != 0, this);
                if (engine is not null)
                {
                    return engine;
                }
            }

            var created = await engineFactory(cancellationToken);
            lock (sync)
            {
                if (disposed == 0)
                {
                    engine = created;
                    return created;
                }
            }

            await created.DisposeAsync();
            throw new ObjectDisposedException(nameof(WindowsHostOperations));
        }
        finally
        {
            engineCreationGate.Release();
        }
    }

    private static string MapHdrCapture(WindowsTargetHdrState? state) =>
        state switch
        {
            WindowsTargetHdrState.Active => "supported",
            WindowsTargetHdrState.Inactive => "unavailable",
            _ => "unvalidated",
        };

    private static bool TryMapDelivery(string value, out OutputTarget delivery)
    {
        delivery = value switch
        {
            "clipboard" => OutputTarget.Clipboard,
            "folder" => OutputTarget.Folder,
            "both" => OutputTarget.Both,
            _ => default,
        };
        return value is "clipboard" or "folder" or "both";
    }

    private HostCaptureResult MapCaptureResult(
        WindowsCaptureResult result,
        OutputTarget requestedDelivery,
        string requestId,
        string captureMode)
    {
        if (result.Outcome == WindowsCaptureOutcome.Cancelled)
        {
            return new HostCaptureResult("cancelled");
        }

        if (result.Output is { } output
            && result.Outcome is WindowsCaptureOutcome.Delivered
                or WindowsCaptureOutcome.DeliveryFailed)
        {
            return Completed(
                result.HdrCapability?.IsHdrActive == true ? "hdr" : "sdr",
                RequestedTargets(requestedDelivery)
                    .Select(target => MapDeliveryResult(
                        output,
                        target,
                        result.TechnicalDetail,
                        requestId))
                    .ToArray());
        }

        logger.LogWarning(
            "operation=CaptureResult, stage={Outcome}, correlation={CorrelationId}, detail={Detail}",
            result.Outcome,
            requestId,
            result.TechnicalDetail);
        return result.Outcome switch
        {
            WindowsCaptureOutcome.TimedOut => Failed(
                "capture-unavailable",
                "Windows capture timed out. Try again.",
                retryable: true),
            WindowsCaptureOutcome.Unavailable => Failed(
                "capture-unavailable",
                captureMode == "region"
                    ? "The frozen Region capture expired. Select the region again."
                    : "The capture target is unavailable. Try again.",
                retryable: true),
            WindowsCaptureOutcome.Unsupported => Failed(
                "capture-unavailable",
                "Screen capture is unavailable on this Windows system.",
                retryable: false),
            _ => Failed(
                "unexpected-failure",
                "Windows capture failed. Try again.",
                retryable: true),
        };
    }

    private HostPrepareRegionResult MapPrepareFailure(
        WindowsPrepareRegionResult result,
        string requestId)
    {
        logger.LogWarning(
            "operation=PrepareRegion, stage={Outcome}, correlation={CorrelationId}, detail={Detail}",
            result.Outcome,
            requestId,
            result.TechnicalDetail);
        return result.Outcome switch
        {
            WindowsCaptureOutcome.TimedOut => PrepareFailed(
                "capture-unavailable",
                "Windows capture timed out. Try again.",
                retryable: true),
            WindowsCaptureOutcome.Unavailable or WindowsCaptureOutcome.Unsupported => PrepareFailed(
                "capture-unavailable",
                "The capture target is unavailable. Try again.",
                retryable: true),
            WindowsCaptureOutcome.Cancelled => PrepareFailed(
                "capture-unavailable",
                "Windows capture timed out. Try again.",
                retryable: true),
            _ => PrepareFailed(
                "unexpected-failure",
                "Windows capture failed. Try again.",
                retryable: true),
        };
    }

    private static HostCaptureResult Completed(
        string sourceDynamicRange,
        IReadOnlyList<HostDeliveryResult> deliveries) =>
        new(
            "completed",
            sourceDynamicRange,
            "srgb-visual-match",
            deliveries);

    private static IReadOnlyList<OutputTarget> RequestedTargets(OutputTarget delivery) =>
        delivery switch
        {
            OutputTarget.Clipboard => [OutputTarget.Clipboard],
            OutputTarget.Folder => [OutputTarget.Folder],
            OutputTarget.Both => [OutputTarget.Clipboard, OutputTarget.Folder],
            _ => throw new ArgumentOutOfRangeException(nameof(delivery), delivery, null),
        };

    private HostDeliveryResult MapDeliveryResult(
        OutputResult output,
        OutputTarget target,
        string fallbackDetail,
        string requestId)
    {
        var targetResult = output.Targets.FirstOrDefault(result => result.Target == target);
        var protocolTarget = target == OutputTarget.Clipboard ? "clipboard" : "folder";
        if (targetResult?.Outcome == OutputOutcome.Success
            && (target != OutputTarget.Folder
                || !string.IsNullOrWhiteSpace(targetResult.ArtifactPath)))
        {
            return new HostDeliveryResult(
                protocolTarget,
                "success",
                target == OutputTarget.Folder ? targetResult.ArtifactPath : null);
        }

        var message = targetResult?.TechnicalDetail
            ?? $"{protocolTarget} delivery did not return a result. {fallbackDetail}";
        logger.LogWarning(
            "operation=CaptureDelivery, stage=Failed, correlation={CorrelationId}, target={Target}, detail={Detail}",
            requestId,
            protocolTarget,
            message);
        return new HostDeliveryResult(
            protocolTarget,
            "failed",
            Failure: new PlatformFailure(
                "delivery-failed",
                target == OutputTarget.Clipboard
                    ? "Could not copy the screenshot to the clipboard."
                    : "Could not save the screenshot.",
                Retryable: true));
    }

    private static HostCaptureResult Failed(
        string code,
        string message,
        bool retryable) =>
        new("failed", Failure: new PlatformFailure(code, message, retryable));

    private static HostPrepareRegionResult PrepareFailed(
        string code,
        string message,
        bool retryable) =>
        new("failed", Failure: new PlatformFailure(code, message, retryable));

    public async ValueTask DisposeAsync()
    {
        if (Interlocked.Exchange(ref disposed, 1) != 0)
        {
            return;
        }

        IWindowsCaptureEngine? ownedEngine;
        lock (sync)
        {
            ownedEngine = engine;
            engine = null;
        }

        if (ownedEngine is not null)
        {
            await ownedEngine.DisposeAsync();
        }
    }

    private sealed class WindowsDisplayCaptureEngineAdapter(WindowsDisplayCaptureEngine inner)
        : IWindowsCaptureEngine
    {
        public Task<WindowsCaptureResult> CaptureDisplayAsync(
            WindowsCaptureRequest request,
            CancellationToken cancellationToken = default) =>
            inner.CaptureDisplayAsync(request, cancellationToken);

        public Task<WindowsPrepareRegionResult> PrepareRegionAsync(
            string requestId,
            WindowsTargetCapability target,
            CancellationToken cancellationToken = default) =>
            inner.PrepareRegionAsync(requestId, target, cancellationToken);

        public Task<WindowsCaptureResult> CommitRegionAsync(
            string sessionId,
            WindowsCaptureRequest request,
            WindowsRegionGeometry geometry,
            CancellationToken cancellationToken = default) =>
            inner.CommitRegionAsync(sessionId, request, geometry, cancellationToken);

        public Task ReleaseRegionAsync(string sessionId) =>
            inner.ReleaseRegionAsync(sessionId);

        public ValueTask DisposeAsync() => inner.DisposeAsync();
    }

    private sealed record IssuedTarget(string Token, WindowsTargetCapability Target);
}

internal static class WindowsCaptureFolder
{
    public static string GetDefaultPath()
    {
        var pictures = Environment.GetFolderPath(Environment.SpecialFolder.MyPictures);
        if (string.IsNullOrWhiteSpace(pictures))
        {
            var profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
            if (string.IsNullOrWhiteSpace(profile))
            {
                throw new InvalidOperationException("The Windows user profile could not be resolved.");
            }

            pictures = Path.Combine(profile, "Pictures");
        }

        return Path.Combine(pictures, "Lumiere");
    }
}
