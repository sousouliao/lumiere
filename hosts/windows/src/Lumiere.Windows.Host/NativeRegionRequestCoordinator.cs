namespace Lumiere.Windows.Host;

/// <summary>
/// Reserves the pending request before capture can yield, so cancellation by request id
/// remains possible throughout acquisition and selection.
/// </summary>
internal sealed class NativeRegionRequestCoordinator : IAsyncDisposable
{
    private readonly object sync = new();
    private ActiveRequest? active;
    private bool disposed;

    public Task<HostCaptureResult> CaptureAsync(
        string requestId,
        Func<CancellationToken, Task<HostCaptureResult>> capture,
        CancellationToken cancellationToken = default)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(requestId);
        ArgumentNullException.ThrowIfNull(capture);

        ActiveRequest request;
        lock (sync)
        {
            ObjectDisposedException.ThrowIf(disposed, this);
            if (active is not null)
            {
                return Task.FromResult(new HostCaptureResult(
                    "failed",
                    Failure: new PlatformFailure(
                        "capture-unavailable",
                        "A Region capture is already in progress.",
                        Retryable: true)));
            }

            request = new ActiveRequest(
                requestId,
                CancellationTokenSource.CreateLinkedTokenSource(cancellationToken));
            active = request;
        }

        return RunAsync(request, capture);
    }

    public async Task<HostReleasedRegion> CancelAsync(string requestId)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(requestId);
        ActiveRequest? request;
        lock (sync)
        {
            request = active?.Id == requestId ? active : null;
            if (request is not null)
            {
                request.Cancellation.Cancel();
            }
        }

        if (request is not null)
        {
            await request.Completion.Task;
        }
        return new HostReleasedRegion();
    }

    private async Task<HostCaptureResult> RunAsync(
        ActiveRequest request,
        Func<CancellationToken, Task<HostCaptureResult>> capture)
    {
        try
        {
            return await capture(request.Cancellation.Token);
        }
        catch (OperationCanceledException) when (request.Cancellation.IsCancellationRequested)
        {
            return new HostCaptureResult("cancelled");
        }
        finally
        {
            lock (sync)
            {
                if (ReferenceEquals(active, request))
                {
                    active = null;
                }
            }

            request.Cancellation.Dispose();
            request.Completion.TrySetResult();
        }
    }

    public async ValueTask DisposeAsync()
    {
        ActiveRequest? request;
        lock (sync)
        {
            if (disposed)
            {
                return;
            }

            disposed = true;
            request = active;
            request?.Cancellation.Cancel();
        }

        if (request is not null)
        {
            await request.Completion.Task;
        }
    }

    private sealed record ActiveRequest(string Id, CancellationTokenSource Cancellation)
    {
        public TaskCompletionSource Completion { get; } =
            new(TaskCreationOptions.RunContinuationsAsynchronously);
    }
}
