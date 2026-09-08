import Foundation

public let platformContractVersion = 4

public enum HostFailureCode: String, Codable, Sendable {
  case hostUnavailable = "host-unavailable"
  case permissionDenied = "permission-denied"
  case captureUnavailable = "capture-unavailable"
  case deliveryUnavailable = "delivery-unavailable"
  case deliveryFailed = "delivery-failed"
  case invalidRequest = "invalid-request"
  case unexpectedFailure = "unexpected-failure"
}

public struct HostFailure: Codable, Equatable, Sendable {
  public let code: HostFailureCode
  public let message: String
  public let retryable: Bool

  public init(code: HostFailureCode, message: String, retryable: Bool) {
    self.code = code
    self.message = message
    self.retryable = retryable
  }
}

public enum CaptureMode: String, Codable, Sendable {
  case region
  case display
}

public enum OutputDelivery: String, Codable, Sendable {
  case clipboard
  case folder
  case both
}

public enum DeliveryTarget: String, Codable, Sendable {
  case clipboard
  case folder
}

public struct LogicalSize: Codable, Equatable, Sendable {
  public let width: Double
  public let height: Double

  public init(width: Double, height: Double) {
    self.width = width
    self.height = height
  }
}

public struct PixelSize: Codable, Equatable, Sendable {
  public let width: Int
  public let height: Int

  public init(width: Int, height: Int) {
    self.width = width
    self.height = height
  }
}

public struct CaptureTarget: Codable, Equatable, Sendable {
  public let id: String
  public let logicalSize: LogicalSize
}

public struct CaptureGeometry: Codable, Equatable, Sendable {
  public let coordinateSpace: String
  public let x: Double
  public let y: Double
  public let width: Double
  public let height: Double

  public init(
    coordinateSpace: String,
    x: Double,
    y: Double,
    width: Double,
    height: Double
  ) {
    self.coordinateSpace = coordinateSpace
    self.x = x
    self.y = y
    self.width = width
    self.height = height
  }
}

public struct DisplayCaptureParameters: Equatable, Sendable {
  public let delivery: OutputDelivery
  public let saveDirectory: String?

  public init(delivery: OutputDelivery, saveDirectory: String? = nil) {
    self.delivery = delivery
    self.saveDirectory = saveDirectory
  }
}

public struct CommitRegionParameters: Equatable, Sendable {
  public let sessionId: String
  public let delivery: OutputDelivery
  public let saveDirectory: String?
  public let geometry: CaptureGeometry
}

public enum PlatformMethod: String, Codable, Sendable {
  case getCapabilities
  case captureDisplay
  case prepareRegion
  case commitRegion
  case cancelRegion
}

public struct PlatformRequest: Equatable, Sendable {
  public let version: Int
  public let id: String
  public let method: PlatformMethod
  public let displayCapture: DisplayCaptureParameters?
  public let commitRegion: CommitRegionParameters?
  public let sessionId: String?
  public let targetId: String?
}

public struct PlatformCapabilities: Codable, Equatable, Sendable {
  public let contractVersion: Int
  public let platform: String
  public let hostStatus: String
  public let captureModes: [CaptureMode]
  public let deliveryTargets: [DeliveryTarget]
  public let hdrCapture: String
  public let outputProfiles: [String]
  public let activeTarget: CaptureTarget?
  public let unavailableReason: HostFailure?

  public init(
    contractVersion: Int,
    platform: String,
    hostStatus: String,
    captureModes: [CaptureMode],
    deliveryTargets: [DeliveryTarget],
    hdrCapture: String,
    outputProfiles: [String],
    activeTarget: CaptureTarget? = nil,
    unavailableReason: HostFailure? = nil
  ) {
    self.contractVersion = contractVersion
    self.platform = platform
    self.hostStatus = hostStatus
    self.captureModes = captureModes
    self.deliveryTargets = deliveryTargets
    self.hdrCapture = hdrCapture
    self.outputProfiles = outputProfiles
    self.activeTarget = activeTarget
    self.unavailableReason = unavailableReason
  }
}

public struct DeliveryResult: Codable, Equatable, Sendable {
  public let target: DeliveryTarget
  public let status: String
  public let filePath: String?
  public let failure: HostFailure?

  public static func clipboardSuccess() -> DeliveryResult {
    DeliveryResult(target: .clipboard, status: "success", filePath: nil, failure: nil)
  }

  public static func folderSuccess(filePath: String) -> DeliveryResult {
    DeliveryResult(target: .folder, status: "success", filePath: filePath, failure: nil)
  }

  public static func failed(target: DeliveryTarget, failure: HostFailure) -> DeliveryResult {
    DeliveryResult(target: target, status: "failed", filePath: nil, failure: failure)
  }
}

public struct CaptureResult: Codable, Equatable, Sendable {
  public let status: String
  public let sourceDynamicRange: String?
  public let outputProfile: String?
  public let deliveries: [DeliveryResult]?
  public let failure: HostFailure?

  public static func completed(dynamicRange: String, deliveries: [DeliveryResult]) -> CaptureResult
  {
    CaptureResult(
      status: "completed",
      sourceDynamicRange: dynamicRange,
      outputProfile: "srgb-visual-match",
      deliveries: deliveries,
      failure: nil
    )
  }

  public static func failed(_ failure: HostFailure) -> CaptureResult {
    CaptureResult(
      status: "failed",
      sourceDynamicRange: nil,
      outputProfile: nil,
      deliveries: nil,
      failure: failure
    )
  }

  public static func cancelled() -> CaptureResult {
    CaptureResult(
      status: "cancelled",
      sourceDynamicRange: nil,
      outputProfile: nil,
      deliveries: nil,
      failure: nil
    )
  }
}

public struct PreparedRegionResult: Codable, Equatable, Sendable {
  public struct Preview: Codable, Equatable, Sendable {
    public let filePath: String
    public let mediaType: String
    public let pixelSize: PixelSize
  }

  public let status: String
  public let sessionId: String
  public let targetLogicalSize: LogicalSize
  public let preview: Preview
  public let leaseMilliseconds: Int
}

public struct ReleasedRegionResult: Codable, Equatable, Sendable {
  public let status: String

  public static let released = ReleasedRegionResult(status: "released")
}

public enum PlatformResult: Equatable, Sendable {
  case capabilities(PlatformCapabilities)
  case capture(CaptureResult)
  case preparedRegion(PreparedRegionResult)
  case releasedRegion(ReleasedRegionResult)
}

public struct PlatformResponse: Encodable, Sendable {
  public let version: Int
  public let id: String
  public let result: PlatformResult?
  public let error: HostFailure?

  public static func success(id: String, result: PlatformResult) -> PlatformResponse {
    PlatformResponse(version: platformContractVersion, id: id, result: result, error: nil)
  }

  public static func failure(id: String, error: HostFailure) -> PlatformResponse {
    PlatformResponse(version: platformContractVersion, id: id, result: nil, error: error)
  }

  private enum CodingKeys: String, CodingKey { case version, id, result, error }

  public func encode(to encoder: Encoder) throws {
    var container = encoder.container(keyedBy: CodingKeys.self)
    try container.encode(version, forKey: .version)
    try container.encode(id, forKey: .id)
    if let error {
      try container.encode(error, forKey: .error)
      return
    }
    switch result {
    case .capabilities(let value): try container.encode(value, forKey: .result)
    case .capture(let value): try container.encode(value, forKey: .result)
    case .preparedRegion(let value): try container.encode(value, forKey: .result)
    case .releasedRegion(let value): try container.encode(value, forKey: .result)
    case nil:
      throw EncodingError.invalidValue(
        self,
        EncodingError.Context(
          codingPath: encoder.codingPath,
          debugDescription: "A platform response requires a result or error."
        )
      )
    }
  }
}

public enum PlatformProtocolError: Error, Equatable, CustomStringConvertible {
  case invalidJSON
  case invalidEnvelope(String)

  public var description: String {
    switch self {
    case .invalidJSON: return "Request must be one complete JSON object."
    case .invalidEnvelope(let message): return message
    }
  }
}

public enum PlatformRequestDecoder {
  public static func decode(line: String) throws -> PlatformRequest {
    guard let data = line.data(using: .utf8),
      let object = try? JSONSerialization.jsonObject(with: data),
      let envelope = object as? [String: Any]
    else { throw PlatformProtocolError.invalidJSON }

    try requireExactKeys(envelope, expected: ["version", "id", "method", "params"])
    guard let version = envelope["version"] as? Int, version == platformContractVersion else {
      throw PlatformProtocolError.invalidEnvelope("Protocol version must be 4.")
    }
    guard let id = envelope["id"] as? String, !id.isEmpty else {
      throw PlatformProtocolError.invalidEnvelope("Request id must be a non-empty string.")
    }
    guard let methodValue = envelope["method"] as? String,
      let method = PlatformMethod(rawValue: methodValue),
      let parameters = envelope["params"] as? [String: Any]
    else { throw PlatformProtocolError.invalidEnvelope("Unknown platform-host method.") }

    switch method {
    case .getCapabilities:
      try requireExactKeys(parameters, expected: [])
      return PlatformRequest(
        version: version,
        id: id,
        method: method,
        displayCapture: nil,
        commitRegion: nil,
        sessionId: nil,
        targetId: nil
      )
    case .prepareRegion:
      try requireExactKeys(parameters, expected: ["targetId"])
      guard let targetId = parameters["targetId"] as? String, !targetId.isEmpty else {
        throw PlatformProtocolError.invalidEnvelope("Region target id must be non-empty.")
      }
      return PlatformRequest(
        version: version, id: id, method: method, displayCapture: nil, commitRegion: nil,
        sessionId: nil, targetId: targetId
      )
    case .captureDisplay:
      return PlatformRequest(
        version: version,
        id: id,
        method: method,
        displayCapture: try decodeDisplayCapture(parameters),
        commitRegion: nil,
        sessionId: nil,
        targetId: nil
      )
    case .commitRegion:
      return PlatformRequest(
        version: version,
        id: id,
        method: method,
        displayCapture: nil,
        commitRegion: try decodeCommitRegion(parameters),
        sessionId: nil,
        targetId: nil
      )
    case .cancelRegion:
      try requireExactKeys(parameters, expected: ["sessionId"])
      guard let sessionId = parameters["sessionId"] as? String, !sessionId.isEmpty else {
        throw PlatformProtocolError.invalidEnvelope(
          "Region session id must be a non-empty string."
        )
      }
      return PlatformRequest(
        version: version,
        id: id,
        method: method,
        displayCapture: nil,
        commitRegion: nil,
        sessionId: sessionId,
        targetId: nil
      )
    }
  }

  private static func decodeDisplayCapture(
    _ parameters: [String: Any]
  ) throws -> DisplayCaptureParameters {
    let (delivery, saveDirectory) = try decodeDelivery(parameters, extraKeys: [])
    return DisplayCaptureParameters(delivery: delivery, saveDirectory: saveDirectory)
  }

  private static func decodeCommitRegion(
    _ parameters: [String: Any]
  ) throws -> CommitRegionParameters {
    let (delivery, saveDirectory) = try decodeDelivery(
      parameters,
      extraKeys: ["sessionId", "geometry"]
    )
    guard let sessionId = parameters["sessionId"] as? String, !sessionId.isEmpty,
      let geometryObject = parameters["geometry"] as? [String: Any]
    else {
      throw PlatformProtocolError.invalidEnvelope(
        "Region commit requires a non-empty session id and geometry."
      )
    }
    return CommitRegionParameters(
      sessionId: sessionId,
      delivery: delivery,
      saveDirectory: saveDirectory,
      geometry: try decodeGeometry(geometryObject)
    )
  }

  private static func decodeDelivery(
    _ parameters: [String: Any],
    extraKeys: Set<String>
  ) throws -> (OutputDelivery, String?) {
    guard let deliveryValue = parameters["delivery"] as? String,
      let delivery = OutputDelivery(rawValue: deliveryValue)
    else {
      throw PlatformProtocolError.invalidEnvelope(
        "Capture delivery must be clipboard, folder, or both."
      )
    }
    let saveDirectory = try decodeSaveDirectory(parameters, delivery: delivery)
    var expected = extraKeys.union(["delivery"])
    if saveDirectory != nil { expected.insert("saveDirectory") }
    try requireExactKeys(parameters, expected: expected)
    return (delivery, saveDirectory)
  }

  private static func decodeSaveDirectory(
    _ parameters: [String: Any],
    delivery: OutputDelivery
  ) throws -> String? {
    guard let value = parameters["saveDirectory"] else { return nil }
    guard delivery != .clipboard else {
      throw PlatformProtocolError.invalidEnvelope(
        "Clipboard-only capture must not include a save directory."
      )
    }
    guard let path = value as? String, !path.isEmpty, path.hasPrefix("/") else {
      throw PlatformProtocolError.invalidEnvelope(
        "Save directory must be an absolute non-empty path."
      )
    }
    return path
  }

  private static func decodeGeometry(_ object: [String: Any]) throws -> CaptureGeometry {
    try requireExactKeys(
      object,
      expected: ["coordinateSpace", "x", "y", "width", "height"]
    )
    guard object["coordinateSpace"] as? String == "target-logical",
      let x = finiteNumber(object["x"]), x >= 0,
      let y = finiteNumber(object["y"]), y >= 0,
      let width = finiteNumber(object["width"]), width > 0,
      let height = finiteNumber(object["height"]), height > 0
    else {
      throw PlatformProtocolError.invalidEnvelope(
        "Region geometry must be a positive target-logical rectangle."
      )
    }
    return CaptureGeometry(
      coordinateSpace: "target-logical",
      x: x,
      y: y,
      width: width,
      height: height
    )
  }

  private static func finiteNumber(_ value: Any?) -> Double? {
    guard let number = value as? NSNumber,
      CFGetTypeID(number) != CFBooleanGetTypeID(),
      number.doubleValue.isFinite
    else { return nil }
    return number.doubleValue
  }

  private static func requireExactKeys(
    _ object: [String: Any],
    expected: Set<String>
  ) throws {
    guard Set(object.keys) == expected else {
      throw PlatformProtocolError.invalidEnvelope("Request contains missing or unknown fields.")
    }
  }
}

public enum PlatformResponseEncoder {
  public static func encodeLine(_ response: PlatformResponse) throws -> String {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
    let data = try encoder.encode(response)
    guard let line = String(data: data, encoding: .utf8) else {
      throw PlatformProtocolError.invalidJSON
    }
    return line
  }
}

public struct HostDiagnostic: Encodable, Equatable, Sendable {
  public let level: String
  public let event: String
  public let requestID: String
  public let code: HostFailureCode
  public let message: String
  public let retryable: Bool

  public init(requestID: String, event: String, failure: HostFailure) {
    self.level = "error"
    self.event = event
    self.requestID = requestID
    self.code = failure.code
    self.message = failure.message
    self.retryable = failure.retryable
  }
}

public enum HostDiagnosticEncoder {
  public static func encodeLine(_ diagnostic: HostDiagnostic) throws -> String {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
    let data = try encoder.encode(diagnostic)
    guard let line = String(data: data, encoding: .utf8) else {
      throw PlatformProtocolError.invalidJSON
    }
    return line
  }
}
