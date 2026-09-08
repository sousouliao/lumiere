using System.Text.Json;
using Lumiere.Windows.Capture;
using Lumiere.Windows.Host;
using Xunit;

namespace Lumiere.Windows.Host.Tests;

public sealed class PlatformProtocolTests
{
    [Fact]
    public async Task GetCapabilities_ReportsImplementedWindowsSlice()
    {
        await using var operations = CreateOperations();
        var response = await PlatformProtocol.ProcessLineAsync(
            """{"version":4,"id":"capabilities-1","method":"getCapabilities","params":{}}""",
            operations);

        using var document = JsonDocument.Parse(response.ResponseLine);
        var root = document.RootElement;
        var result = root.GetProperty("result");
        Assert.Equal(4, root.GetProperty("version").GetInt32());
        Assert.Equal("capabilities-1", root.GetProperty("id").GetString());
        Assert.Equal("windows", result.GetProperty("platform").GetString());
        Assert.Equal("available", result.GetProperty("hostStatus").GetString());
        Assert.Equal("display", result.GetProperty("captureModes")[0].GetString());
        Assert.Equal("clipboard", result.GetProperty("deliveryTargets")[0].GetString());
        Assert.Equal("folder", result.GetProperty("deliveryTargets")[1].GetString());
        Assert.Equal("unvalidated", result.GetProperty("hdrCapture").GetString());
        Assert.Equal("srgb-visual-match", result.GetProperty("outputProfiles")[0].GetString());
        Assert.False(result.TryGetProperty("activeTarget", out _));
        Assert.Null(response.Diagnostic);
    }

    [Fact]
    public async Task CaptureDisplay_SerializesCompletedFolderDelivery()
    {
        var engine = new StubCaptureEngine
        {
            Result = TestCaptureResults.FolderSuccess("C:\\Pictures\\Lumiere\\capture.png"),
        };
        await using var operations = CreateOperations(engine);
        var response = await PlatformProtocol.ProcessLineAsync(
            """{"version":4,"id":"capture-1","method":"captureDisplay","params":{"delivery":"folder"}}""",
            operations);

        using var document = JsonDocument.Parse(response.ResponseLine);
        var result = document.RootElement.GetProperty("result");
        Assert.Equal("completed", result.GetProperty("status").GetString());
        Assert.Equal("sdr", result.GetProperty("sourceDynamicRange").GetString());
        Assert.Equal("srgb-visual-match", result.GetProperty("outputProfile").GetString());
        Assert.Equal("folder", result.GetProperty("deliveries")[0].GetProperty("target").GetString());
        Assert.Equal(
            "C:\\Pictures\\Lumiere\\capture.png",
            result.GetProperty("deliveries")[0].GetProperty("filePath").GetString());
        Assert.Null(response.Diagnostic);
    }

    [Fact]
    public async Task CaptureDisplay_ForwardsCustomSaveDirectory()
    {
        var engine = new StubCaptureEngine
        {
            Result = TestCaptureResults.FolderSuccess("D:\\Screenshots\\capture.png"),
        };
        await using var operations = CreateOperations(engine);

        var response = await PlatformProtocol.ProcessLineAsync(
            """{"version":4,"id":"capture-custom","method":"captureDisplay","params":{"delivery":"folder","saveDirectory":"D:\\Screenshots"}}""",
            operations);

        using var document = JsonDocument.Parse(response.ResponseLine);
        Assert.Equal("completed", document.RootElement.GetProperty("result").GetProperty("status").GetString());
        Assert.Equal("D:\\Screenshots", engine.Request?.SaveDirectory);
    }

    [Fact]
    public async Task CaptureDisplay_RejectsSaveDirectoryForClipboardOnlyRequest()
    {
        await using var operations = CreateOperations();
        var response = await PlatformProtocol.ProcessLineAsync(
            """{"version":4,"id":"capture-invalid-directory","method":"captureDisplay","params":{"delivery":"clipboard","saveDirectory":"D:\\Screenshots"}}""",
            operations);

        using var document = JsonDocument.Parse(response.ResponseLine);
        Assert.Equal(
            "invalid-request",
            document.RootElement.GetProperty("error").GetProperty("code").GetString());
    }

    [Fact]
    public async Task CaptureDisplay_SerializesBothDeliveryOutcomes()
    {
        var engine = new StubCaptureEngine
        {
            Result = TestCaptureResults.BothPartialSuccess(
                "C:\\Pictures\\Lumiere\\capture.png"),
        };
        await using var operations = CreateOperations(engine);
        var response = await PlatformProtocol.ProcessLineAsync(
            """{"version":4,"id":"capture-both","method":"captureDisplay","params":{"delivery":"both"}}""",
            operations);

        using var document = JsonDocument.Parse(response.ResponseLine);
        var deliveries = document.RootElement.GetProperty("result").GetProperty("deliveries");
        Assert.Equal("clipboard", deliveries[0].GetProperty("target").GetString());
        Assert.Equal("failed", deliveries[0].GetProperty("status").GetString());
        Assert.Equal(
            "delivery-failed",
            deliveries[0].GetProperty("failure").GetProperty("code").GetString());
        Assert.Equal("folder", deliveries[1].GetProperty("target").GetString());
        Assert.Equal("success", deliveries[1].GetProperty("status").GetString());
        Assert.Equal(
            "C:\\Pictures\\Lumiere\\capture.png",
            deliveries[1].GetProperty("filePath").GetString());
        Assert.Equal("delivery-failed", response.Diagnostic?.Failure.Code);
    }

    [Fact]
    public async Task GetCapabilities_SerializesHdrWithoutActiveTarget()
    {
        await using var operations = CreateOperations(
            targetCapability: new WindowsTargetCapability(
                WindowsTargetHdrState.Active,
                new WindowsTargetLogicalSize(2560, 1440)));
        var response = await PlatformProtocol.ProcessLineAsync(
            """{"version":4,"id":"capabilities-2","method":"getCapabilities","params":{}}""",
            operations);

        using var document = JsonDocument.Parse(response.ResponseLine);
        var result = document.RootElement.GetProperty("result");
        Assert.Equal("supported", result.GetProperty("hdrCapture").GetString());
        Assert.Equal("display", result.GetProperty("captureModes")[0].GetString());
        Assert.False(result.TryGetProperty("activeTarget", out _));
    }

    [Fact]
    public async Task RegionCapture_PreparesThenCommitsFrozenGeometry()
    {
        var engine = new StubCaptureEngine
        {
            Result = TestCaptureResults.ClipboardSuccess(),
        };
        await using var operations = CreateOperations(
            engine,
            WindowsHostOperationsTests.CreateRegionCapability());
        var capabilitiesResponse = await PlatformProtocol.ProcessLineAsync(
            """{"version":4,"id":"capabilities-region","method":"getCapabilities","params":{}}""",
            operations);
        using var capabilitiesDocument = JsonDocument.Parse(capabilitiesResponse.ResponseLine);
        var activeTarget = capabilitiesDocument.RootElement
            .GetProperty("result")
            .GetProperty("activeTarget");
        Assert.Equal("target-17", activeTarget.GetProperty("id").GetString());

        var preparedResponse = await PlatformProtocol.ProcessLineAsync(
            """{"version":4,"id":"prepare-region","method":"prepareRegion","params":{"targetId":"target-17"}}""",
            operations);
        using var preparedDocument = JsonDocument.Parse(preparedResponse.ResponseLine);
        var prepared = preparedDocument.RootElement.GetProperty("result");
        Assert.Equal("prepared", prepared.GetProperty("status").GetString());
        var sessionId = prepared.GetProperty("sessionId").GetString();
        Assert.False(string.IsNullOrWhiteSpace(sessionId));
        Assert.Equal("image/png", prepared.GetProperty("preview").GetProperty("mediaType").GetString());

        var commitRequest =
            """{"version":4,"id":"commit-region","method":"commitRegion","params":{"sessionId":"__SESSION_ID__","delivery":"clipboard","geometry":{"coordinateSpace":"target-logical","x":12.5,"y":20,"width":300,"height":200}}}"""
                .Replace("__SESSION_ID__", sessionId, StringComparison.Ordinal);
        var response = await PlatformProtocol.ProcessLineAsync(commitRequest, operations);

        using var document = JsonDocument.Parse(response.ResponseLine);
        Assert.Equal("completed", document.RootElement.GetProperty("result").GetProperty("status").GetString());
        Assert.Equal(new WindowsRegionGeometry(12.5, 20, 300, 200), engine.RegionGeometry);
    }

    [Fact]
    public async Task CancelRegion_ReleasesUnknownSession()
    {
        await using var operations = CreateOperations();
        var response = await PlatformProtocol.ProcessLineAsync(
            """{"version":4,"id":"cancel-1","method":"cancelRegion","params":{"sessionId":"missing-session"}}""",
            operations);

        using var document = JsonDocument.Parse(response.ResponseLine);
        Assert.Equal("released", document.RootElement.GetProperty("result").GetProperty("status").GetString());
    }

    [Fact]
    public async Task InvalidRequest_PreservesValidRequestIdForCorrelation()
    {
        await using var operations = CreateOperations();
        var response = await PlatformProtocol.ProcessLineAsync(
            """{"version":4,"id":"bad-1","method":"getCapabilities","params":{},"extra":true}""",
            operations);

        using var document = JsonDocument.Parse(response.ResponseLine);
        var root = document.RootElement;
        Assert.Equal("bad-1", root.GetProperty("id").GetString());
        Assert.Equal("invalid-request", root.GetProperty("error").GetProperty("code").GetString());
        Assert.Equal("bad-1", response.Diagnostic?.RequestId);
    }

    [Theory]
    [InlineData("not-json")]
    [InlineData("{\"version\":2,\"id\":\"bad-2\",\"method\":\"getCapabilities\",\"params\":{}}")]
    [InlineData("{\"version\":3,\"id\":\"bad-3\",\"method\":\"unknown\",\"params\":{}}")]
    [InlineData("{\"version\":3,\"id\":\"bad-4\",\"method\":\"capture\",\"params\":{\"delivery\":\"folder\"}}")]
    [InlineData("{\"version\":3,\"id\":\"bad-5\",\"method\":\"commitRegion\",\"params\":{\"delivery\":\"both\"}}")]
    public async Task InvalidRequests_ReturnProtocolErrors(string line)
    {
        await using var operations = CreateOperations();
        var response = await PlatformProtocol.ProcessLineAsync(line, operations);

        using var document = JsonDocument.Parse(response.ResponseLine);
        Assert.Equal(
            "invalid-request",
            document.RootElement.GetProperty("error").GetProperty("code").GetString());
        Assert.False(document.RootElement.GetProperty("error").GetProperty("retryable").GetBoolean());
        Assert.NotNull(response.Diagnostic);
    }

    private static WindowsHostOperations CreateOperations(
        StubCaptureEngine? engine = null,
        WindowsTargetCapability? targetCapability = null) =>
        new(
            () => engine ?? new StubCaptureEngine(),
            () => targetCapability,
            () => "C:\\Pictures\\Lumiere",
            _ => { },
            targetTokenFactory: () => "target-17");
}
