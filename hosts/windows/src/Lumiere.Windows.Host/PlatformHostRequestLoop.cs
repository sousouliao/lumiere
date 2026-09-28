namespace Lumiere.Windows.Host;

/// <summary>
/// Keeps stdin responsive while a native Region selection is pending. Responses may
/// complete out of order and are serialized so JSON Lines never interleave.
/// </summary>
internal static class PlatformHostRequestLoop
{
    public static async Task RunAsync(
        TextReader input,
        TextWriter output,
        IWindowsHostOperations operations,
        Action<HostDiagnostic> reportDiagnostic)
    {
        ArgumentNullException.ThrowIfNull(input);
        ArgumentNullException.ThrowIfNull(output);
        ArgumentNullException.ThrowIfNull(operations);
        ArgumentNullException.ThrowIfNull(reportDiagnostic);

        using var responseGate = new SemaphoreSlim(1, 1);
        using var lifetime = new CancellationTokenSource();
        var pendingRegionResponses = new List<Task>();

        while (await input.ReadLineAsync() is { } line)
        {
            // ProcessLineAsync reserves the Region id before its first await. The
            // following line can therefore cancel even an acquisition in progress.
            var response = HandleLineAsync(line);
            if (PlatformProtocol.IsPendingNativeRegionRequest(line))
            {
                pendingRegionResponses.RemoveAll(static task => task.IsCompletedSuccessfully);
                pendingRegionResponses.Add(response);
            }
            else
            {
                await response;
            }
        }

        lifetime.Cancel();
        await Task.WhenAll(pendingRegionResponses);

        async Task HandleLineAsync(string line)
        {
            var result = await PlatformProtocol.ProcessLineAsync(line, operations, lifetime.Token);
            await responseGate.WaitAsync();
            try
            {
                await output.WriteLineAsync(result.ResponseLine);
                await output.FlushAsync();
            }
            finally
            {
                responseGate.Release();
            }

            if (result.Diagnostic is { } diagnostic)
            {
                reportDiagnostic(diagnostic);
            }
        }
    }
}
