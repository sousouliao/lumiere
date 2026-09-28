using System.Text.Json;
using Lumiere.Windows.Capture;
using Lumiere.Windows.Host;
using Xunit;

namespace Lumiere.Windows.Host.Tests;

public sealed class NativeRegionProtocolTests
{
    [Fact]
    public async Task V6CapabilityDoesNotAdvertiseUnconnectedNativeRegion()
    {
        await using var operations = new WindowsHostOperations(
            () => new StubCaptureEngine(),
            WindowsHostOperationsTests.CreateRegionCapability,
            () => "C:\\Pictures\\Lumiere",
            _ => { });
        var response = await PlatformProtocol.ProcessLineAsync(
            """{"version":6,"id":"capabilities","method":"getCapabilities","params":{}}""",
            operations);
        using var document = JsonDocument.Parse(response.ResponseLine);
        var result = document.RootElement.GetProperty("result");
        Assert.Equal(6, result.GetProperty("contractVersion").GetInt32());
        Assert.Equal("display", result.GetProperty("captureModes")[0].GetString());
        Assert.Equal(1, result.GetProperty("captureModes").GetArrayLength());
        Assert.False(result.TryGetProperty("activeTarget", out _));
    }

    [Fact]
    public async Task PendingCaptureAcceptsRequestIdCancellationBeforeReturning()
    {
        var entered = false;
        await using var operations = CreateOperations(async (_, _, token) =>
        {
            entered = true;
            await Task.Delay(Timeout.InfiniteTimeSpan, token);
            throw new InvalidOperationException("Cancellation must end selection.");
        });
        using var input = new StringReader(string.Join('\n',
            """{"version":6,"id":"capture-1","method":"captureRegion","params":{"delivery":"clipboard"}}""",
            """{"version":6,"id":"cancel-1","method":"cancelRegion","params":{"requestId":"capture-1"}}"""));
        using var output = new StringWriter();

        await PlatformHostRequestLoop.RunAsync(input, output, operations, _ => { });

        Assert.True(entered);
        var results = output.ToString().Split('\n', StringSplitOptions.RemoveEmptyEntries)
            .Select(static line => JsonDocument.Parse(line)).ToArray();
        try
        {
            Assert.Equal(2, results.Length);
            var capture = results.Single(result => result.RootElement.GetProperty("id").GetString() == "capture-1");
            var cancel = results.Single(result => result.RootElement.GetProperty("id").GetString() == "cancel-1");
            Assert.Equal(6, capture.RootElement.GetProperty("version").GetInt32());
            Assert.Equal("cancelled", capture.RootElement.GetProperty("result").GetProperty("status").GetString());
            Assert.Equal("released", cancel.RootElement.GetProperty("result").GetProperty("status").GetString());
        }
        finally
        {
            foreach (var result in results) result.Dispose();
        }
    }

    [Fact]
    public async Task ClosingInputCancelsPendingCaptureBeforeHostDisposal()
    {
        await using var operations = CreateOperations(async (_, _, token) =>
        {
            await Task.Delay(Timeout.InfiniteTimeSpan, token);
            throw new InvalidOperationException("Closing stdin must cancel selection.");
        });
        using var input = new StringReader(
            """{"version":6,"id":"capture-1","method":"captureRegion","params":{"delivery":"clipboard"}}""");
        using var output = new StringWriter();

        await PlatformHostRequestLoop.RunAsync(input, output, operations, _ => { });

        using var response = JsonDocument.Parse(output.ToString());
        Assert.Equal("cancelled", response.RootElement.GetProperty("result").GetProperty("status").GetString());
    }

    [Fact]
    public async Task SecondNativeRegionRequestFailsWhileFirstIsPending()
    {
        await using var operations = CreateOperations(async (_, _, token) =>
        {
            await Task.Delay(Timeout.InfiniteTimeSpan, token);
            throw new InvalidOperationException("Cancellation must end selection.");
        });
        var first = PlatformProtocol.ProcessLineAsync(
            """{"version":6,"id":"capture-1","method":"captureRegion","params":{"delivery":"clipboard"}}""",
            operations);
        var second = await PlatformProtocol.ProcessLineAsync(
            """{"version":6,"id":"capture-2","method":"captureRegion","params":{"delivery":"clipboard"}}""",
            operations);
        using var secondResponse = JsonDocument.Parse(second.ResponseLine);
        Assert.Equal("capture-unavailable",
            secondResponse.RootElement.GetProperty("result").GetProperty("failure").GetProperty("code").GetString());

        var cancelled = await PlatformProtocol.ProcessLineAsync(
            """{"version":6,"id":"cancel-1","method":"cancelRegion","params":{"requestId":"capture-1"}}""",
            operations);
        Assert.NotNull(cancelled);
        using var firstResponse = JsonDocument.Parse((await first).ResponseLine);
        Assert.Equal("cancelled", firstResponse.RootElement.GetProperty("result").GetProperty("status").GetString());
    }

    [Theory]
    [InlineData("""{"version":6,"id":"old","method":"prepareRegion","params":{"targetId":"token"}}""")]
    [InlineData("""{"version":6,"id":"old","method":"commitRegion","params":{}}""")]
    [InlineData("""{"version":6,"id":"old","method":"cancelRegion","params":{"sessionId":"session"}}""")]
    public async Task V6RejectsPreviewSessionMethods(string line)
    {
        await using var operations = CreateOperations((_, _, _) =>
            Task.FromResult(new HostCaptureResult("cancelled")));
        var response = await PlatformProtocol.ProcessLineAsync(line, operations);
        using var document = JsonDocument.Parse(response.ResponseLine);
        Assert.Equal(6, document.RootElement.GetProperty("version").GetInt32());
        Assert.Equal("invalid-request", document.RootElement.GetProperty("error").GetProperty("code").GetString());
    }

    private static WindowsHostOperations CreateOperations(
        Func<string, HostCaptureRequest, CancellationToken, Task<HostCaptureResult>> capture) =>
        new(
            () => new StubCaptureEngine(),
            () => null,
            () => "C:\\Pictures\\Lumiere",
            _ => { },
            nativeRegionCapture: capture);
}
