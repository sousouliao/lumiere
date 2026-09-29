using System.Text.Json;
using System.Text.Json.Serialization;

namespace Lumiere.Windows.Host;

public sealed record PlatformFailure(string Code, string Message, bool Retryable);

public sealed record HostDiagnostic(
    string Event,
    string RequestId,
    PlatformFailure Failure);

public sealed record ProtocolLineResult(
    string ResponseLine,
    HostDiagnostic? Diagnostic = null);

public sealed record HostCaptureGeometry(double X, double Y, double Width, double Height);

public sealed record HostCaptureRequest(string Delivery, string? SaveDirectory = null);

public sealed record HostCommitRegionRequest(
    string SessionId,
    string Delivery,
    HostCaptureGeometry Geometry,
    string? SaveDirectory = null);

public sealed record HostLogicalSize(double Width, double Height);

public sealed record HostCaptureTarget(string Id, HostLogicalSize LogicalSize);

public sealed record HostPixelSize(int Width, int Height);

public sealed record HostRegionPreview(string FilePath, string MediaType, HostPixelSize PixelSize);

public sealed record HostCapabilities(
    int ContractVersion,
    string Platform,
    string HostStatus,
    IReadOnlyList<string> CaptureModes,
    IReadOnlyList<string> DeliveryTargets,
    string HdrCapture,
    IReadOnlyList<string> OutputProfiles,
    HostCaptureTarget? ActiveTarget = null);

public sealed record HostDeliveryResult(
    string Target,
    string Status,
    string? FilePath = null,
    PlatformFailure? Failure = null);

public sealed record HostCaptureResult(
    string Status,
    string? SourceDynamicRange = null,
    string? OutputProfile = null,
    IReadOnlyList<HostDeliveryResult>? Deliveries = null,
    PlatformFailure? Failure = null);

public sealed record HostPrepareRegionResult(
    string Status,
    string? SessionId = null,
    HostLogicalSize? TargetLogicalSize = null,
    HostRegionPreview? Preview = null,
    int? LeaseMilliseconds = null,
    PlatformFailure? Failure = null);

public sealed record HostReleasedRegion(string Status = "released");

public interface IWindowsHostOperations : IAsyncDisposable
{
    HostCapabilities GetCapabilities();

    HostCapabilities GetNativeRegionCapabilities();

    Task<HostCaptureResult> CaptureDisplayAsync(
        string requestId,
        HostCaptureRequest request,
        CancellationToken cancellationToken = default);

    Task<HostPrepareRegionResult> PrepareRegionAsync(
        string requestId,
        string targetId,
        CancellationToken cancellationToken = default);

    Task<HostCaptureResult> CommitRegionAsync(
        string requestId,
        HostCommitRegionRequest request,
        CancellationToken cancellationToken = default);

    Task<HostReleasedRegion> CancelRegionAsync(string sessionId);

    Task<HostCaptureResult> CaptureNativeRegionAsync(
        string requestId,
        HostCaptureRequest request,
        CancellationToken cancellationToken = default);

    Task<HostReleasedRegion> CancelNativeRegionAsync(string requestId);
}

public static class PlatformProtocol
{
    public const int ContractVersion = 5;
    public const int NativeRegionContractVersion = 6;

    private static readonly JsonSerializerOptions SerializerOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
    };

    public static bool IsPendingNativeRegionRequest(string line)
    {
        try
        {
            using var document = JsonDocument.Parse(line);
            var envelope = document.RootElement;
            return envelope.ValueKind == JsonValueKind.Object
                && envelope.TryGetProperty("version", out var version)
                && version.ValueKind == JsonValueKind.Number
                && version.TryGetInt32(out var value)
                && value == NativeRegionContractVersion
                && envelope.TryGetProperty("method", out var method)
                && method.ValueKind == JsonValueKind.String
                && method.GetString() == "captureRegion";
        }
        catch (JsonException)
        {
            return false;
        }
    }

    public static async Task<ProtocolLineResult> ProcessLineAsync(
        string line,
        IWindowsHostOperations operations,
        CancellationToken cancellationToken = default)
    {
        ArgumentNullException.ThrowIfNull(line);
        ArgumentNullException.ThrowIfNull(operations);

        var requestId = "invalid-request";
        var responseVersion = ContractVersion;
        try
        {
            using var document = JsonDocument.Parse(line);
            var envelope = document.RootElement;
            if (envelope.ValueKind == JsonValueKind.Object
                && envelope.TryGetProperty("id", out var candidateId)
                && candidateId.ValueKind == JsonValueKind.String
                && !string.IsNullOrEmpty(candidateId.GetString()))
            {
                requestId = candidateId.GetString()!;
            }
            if (envelope.ValueKind == JsonValueKind.Object
                && envelope.TryGetProperty("version", out var candidateVersion)
                && candidateVersion.ValueKind == JsonValueKind.Number
                && candidateVersion.TryGetInt32(out var requestedVersion)
                && requestedVersion == NativeRegionContractVersion)
            {
                responseVersion = NativeRegionContractVersion;
            }

            RequireExactProperties(envelope, "version", "id", "method", "params");
            var version = RequireVersion(envelope);
            requestId = RequireNonEmptyString(envelope, "id", "Request id");
            var method = RequireNonEmptyString(envelope, "method", "Request method");
            var parameters = RequireObject(envelope, "params", "Request params");

            return (version, method) switch
            {
                (_, "getCapabilities") => ProcessGetCapabilities(version, requestId, parameters, operations),
                (_, "captureDisplay") => await ProcessCaptureDisplayAsync(
                    version,
                    requestId,
                    parameters,
                    operations,
                    cancellationToken),
                (ContractVersion, "prepareRegion") => await ProcessPrepareRegionAsync(
                    requestId,
                    parameters,
                    operations,
                    cancellationToken),
                (ContractVersion, "commitRegion") => await ProcessCommitRegionAsync(
                    requestId,
                    parameters,
                    operations,
                    cancellationToken),
                (ContractVersion, "cancelRegion") => await ProcessCancelRegionAsync(requestId, parameters, operations),
                (NativeRegionContractVersion, "captureRegion") => await ProcessCaptureNativeRegionAsync(
                    requestId,
                    parameters,
                    operations,
                    cancellationToken),
                (NativeRegionContractVersion, "cancelRegion") => await ProcessCancelNativeRegionAsync(
                    requestId,
                    parameters,
                    operations),
                _ => throw new PlatformProtocolException("Unknown platform-host method."),
            };
        }
        catch (JsonException)
        {
            return Failure(
                responseVersion,
                requestId,
                "invalid-request",
                "Request must be one complete JSON object.",
                retryable: false);
        }
        catch (PlatformProtocolException exception)
        {
            return Failure(
                responseVersion,
                requestId,
                "invalid-request",
                exception.Message,
                retryable: false);
        }
    }

    private static ProtocolLineResult ProcessGetCapabilities(
        int version,
        string requestId,
        JsonElement parameters,
        IWindowsHostOperations operations)
    {
        RequireExactProperties(parameters);
        var capabilities = version == NativeRegionContractVersion
            ? operations.GetNativeRegionCapabilities()
            : operations.GetCapabilities();
        return Success(version, requestId, capabilities);
    }

    private static async Task<ProtocolLineResult> ProcessCaptureDisplayAsync(
        int version,
        string requestId,
        JsonElement parameters,
        IWindowsHostOperations operations,
        CancellationToken cancellationToken)
    {
        var request = ValidateDisplayParameters(parameters);
        var result = await operations.CaptureDisplayAsync(requestId, request, cancellationToken);
        return CaptureResult(version, requestId, result);
    }

    private static async Task<ProtocolLineResult> ProcessPrepareRegionAsync(
        string requestId,
        JsonElement parameters,
        IWindowsHostOperations operations,
        CancellationToken cancellationToken)
    {
        RequireExactProperties(parameters, "targetId");
        var targetId = RequireNonEmptyString(parameters, "targetId", "Region target id");
        var result = await operations.PrepareRegionAsync(requestId, targetId, cancellationToken);
        var failure = result.Failure;
        return new ProtocolLineResult(
            Serialize(new { version = ContractVersion, id = requestId, result }),
            failure is null ? null : new HostDiagnostic(failure.Code, requestId, failure));
    }

    private static async Task<ProtocolLineResult> ProcessCommitRegionAsync(
        string requestId,
        JsonElement parameters,
        IWindowsHostOperations operations,
        CancellationToken cancellationToken)
    {
        var request = ValidateCommitRegionParameters(parameters);
        var result = await operations.CommitRegionAsync(requestId, request, cancellationToken);
        return CaptureResult(ContractVersion, requestId, result);
    }

    private static async Task<ProtocolLineResult> ProcessCancelRegionAsync(
        string requestId,
        JsonElement parameters,
        IWindowsHostOperations operations)
    {
        RequireExactProperties(parameters, "sessionId");
        var sessionId = RequireNonEmptyString(parameters, "sessionId", "Region session id");
        var result = await operations.CancelRegionAsync(sessionId);
        return Success(ContractVersion, requestId, result);
    }

    private static async Task<ProtocolLineResult> ProcessCaptureNativeRegionAsync(
        string requestId,
        JsonElement parameters,
        IWindowsHostOperations operations,
        CancellationToken cancellationToken)
    {
        var request = ValidateDisplayParameters(parameters);
        var result = await operations.CaptureNativeRegionAsync(requestId, request, cancellationToken);
        return CaptureResult(NativeRegionContractVersion, requestId, result);
    }

    private static async Task<ProtocolLineResult> ProcessCancelNativeRegionAsync(
        string requestId,
        JsonElement parameters,
        IWindowsHostOperations operations)
    {
        RequireExactProperties(parameters, "requestId");
        var captureRequestId = RequireNonEmptyString(parameters, "requestId", "Region request id");
        return Success(
            NativeRegionContractVersion,
            requestId,
            await operations.CancelNativeRegionAsync(captureRequestId));
    }

    private static ProtocolLineResult CaptureResult(
        int version,
        string requestId,
        HostCaptureResult result)
    {
        var failure = result.Failure
            ?? result.Deliveries?.FirstOrDefault(delivery => delivery.Failure is not null)?.Failure;
        return new ProtocolLineResult(
            Serialize(new { version, id = requestId, result }),
            failure is null
                ? null
                : new HostDiagnostic(failure.Code, requestId, failure));
    }

    private static HostCaptureRequest ValidateDisplayParameters(JsonElement parameters)
    {
        var delivery = RequireNonEmptyString(parameters, "delivery", "Capture delivery");
        RequireSupportedDelivery(delivery);
        var saveDirectory = ReadSaveDirectory(parameters, delivery);
        RequireExactProperties(
            parameters,
            saveDirectory is null
                ? ["delivery"]
                : ["delivery", "saveDirectory"]);
        return new HostCaptureRequest(delivery, saveDirectory);
    }

    private static HostCommitRegionRequest ValidateCommitRegionParameters(JsonElement parameters)
    {
        var sessionId = RequireNonEmptyString(parameters, "sessionId", "Region session id");
        var delivery = RequireNonEmptyString(parameters, "delivery", "Capture delivery");
        RequireSupportedDelivery(delivery);
        var saveDirectory = ReadSaveDirectory(parameters, delivery);
        RequireExactProperties(
            parameters,
            saveDirectory is null
                ? ["sessionId", "delivery", "geometry"]
                : ["sessionId", "delivery", "geometry", "saveDirectory"]);
        var geometry = RequireObject(parameters, "geometry", "Region geometry");
        RequireExactProperties(geometry, "coordinateSpace", "x", "y", "width", "height");
        if (RequireNonEmptyString(geometry, "coordinateSpace", "Region coordinate space")
            != "target-logical")
        {
            throw new PlatformProtocolException(
                "Region geometry must use target-logical coordinates.");
        }

        return new HostCommitRegionRequest(
            sessionId,
            delivery,
            new HostCaptureGeometry(
                RequireFiniteNumber(geometry, "x", positive: false),
                RequireFiniteNumber(geometry, "y", positive: false),
                RequireFiniteNumber(geometry, "width", positive: true),
                RequireFiniteNumber(geometry, "height", positive: true)),
            saveDirectory);
    }

    private static void RequireSupportedDelivery(string delivery)
    {
        if (delivery is not ("clipboard" or "folder" or "both"))
        {
            throw new PlatformProtocolException(
                "Capture delivery must be clipboard, folder, or both.");
        }
    }

    private static string? ReadSaveDirectory(JsonElement parameters, string delivery)
    {
        if (!parameters.TryGetProperty("saveDirectory", out var property))
        {
            return null;
        }

        if (delivery == "clipboard")
        {
            throw new PlatformProtocolException(
                "Clipboard-only capture must not include a save directory.");
        }

        if (property.ValueKind != JsonValueKind.String
            || string.IsNullOrWhiteSpace(property.GetString())
            || !Path.IsPathFullyQualified(property.GetString()!))
        {
            throw new PlatformProtocolException(
                "Save directory must be an absolute non-empty path.");
        }

        return property.GetString();
    }

    private static ProtocolLineResult Success(int version, string requestId, object result) =>
        new(Serialize(new { version, id = requestId, result }));

    private static ProtocolLineResult Failure(
        int version,
        string requestId,
        string code,
        string message,
        bool retryable)
    {
        var failure = new PlatformFailure(code, message, retryable);
        return new ProtocolLineResult(
            Serialize(new { version, id = requestId, error = failure }),
            new HostDiagnostic("request-failed", requestId, failure));
    }

    private static string Serialize(object value) =>
        JsonSerializer.Serialize(value, SerializerOptions);

    private static int RequireVersion(JsonElement envelope)
    {
        if (!envelope.TryGetProperty("version", out var version)
            || version.ValueKind != JsonValueKind.Number
            || !version.TryGetInt32(out var value)
            || value is not (ContractVersion or NativeRegionContractVersion))
        {
            throw new PlatformProtocolException("Protocol version must be 5 or 6.");
        }

        return value;
    }

    private static JsonElement RequireObject(
        JsonElement value,
        string propertyName,
        string description)
    {
        if (!value.TryGetProperty(propertyName, out var property)
            || property.ValueKind != JsonValueKind.Object)
        {
            throw new PlatformProtocolException($"{description} must be an object.");
        }

        return property;
    }

    private static string RequireNonEmptyString(
        JsonElement value,
        string propertyName,
        string description)
    {
        if (!value.TryGetProperty(propertyName, out var property)
            || property.ValueKind != JsonValueKind.String
            || string.IsNullOrEmpty(property.GetString()))
        {
            throw new PlatformProtocolException($"{description} must be a non-empty string.");
        }

        return property.GetString()!;
    }

    private static double RequireFiniteNumber(
        JsonElement value,
        string propertyName,
        bool positive)
    {
        if (!value.TryGetProperty(propertyName, out var property)
            || property.ValueKind != JsonValueKind.Number
            || !property.TryGetDouble(out var number)
            || !double.IsFinite(number)
            || (positive ? number <= 0 : number < 0))
        {
            var requirement = positive ? "a finite positive number" : "a finite non-negative number";
            throw new PlatformProtocolException(
                $"Region {propertyName} must be {requirement}.");
        }

        return number;
    }

    private static void RequireExactProperties(
        JsonElement value,
        params string[] expectedProperties)
    {
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new PlatformProtocolException("Request value must be an object.");
        }

        var expected = expectedProperties.ToHashSet(StringComparer.Ordinal);
        var actual = new HashSet<string>(StringComparer.Ordinal);
        foreach (var property in value.EnumerateObject())
        {
            if (!actual.Add(property.Name) || !expected.Contains(property.Name))
            {
                throw new PlatformProtocolException(
                    "Request contains missing, duplicate, or unknown fields.");
            }
        }

        if (!actual.SetEquals(expected))
        {
            throw new PlatformProtocolException(
                "Request contains missing, duplicate, or unknown fields.");
        }
    }

    private sealed class PlatformProtocolException(string message) : Exception(message);
}
