import CoreGraphics
import Foundation
import ImageIO
import ScreenCaptureKit
import Testing

@testable import LumiereMacHostCore

@Test
func decodesGetCapabilitiesRequest() throws {
  let request = try PlatformRequestDecoder.decode(
    line: #"{"version":4,"id":"capabilities-1","method":"getCapabilities","params":{}}"#
  )

  #expect(request.version == 4)
  #expect(request.id == "capabilities-1")
  #expect(request.method == .getCapabilities)
  #expect(request.displayCapture == nil)
  #expect(request.commitRegion == nil)
}

@Test
func decodesDisplayFolderCaptureRequest() throws {
  let request = try PlatformRequestDecoder.decode(
    line:
      #"{"version":4,"id":"capture-1","method":"captureDisplay","params":{"delivery":"folder"}}"#
  )

  #expect(request.displayCapture == DisplayCaptureParameters(delivery: .folder))
}

@Test
func decodesCustomSaveDirectoryForFolderCapture() throws {
  let request = try PlatformRequestDecoder.decode(
    line:
      #"{"version":4,"id":"capture-custom","method":"captureDisplay","params":{"delivery":"folder","saveDirectory":"/tmp/custom-captures"}}"#
  )

  #expect(request.displayCapture?.saveDirectory == "/tmp/custom-captures")
}

@Test
func rejectsSaveDirectoryForClipboardOnlyCapture() {
  #expect(throws: PlatformProtocolError.self) {
    try PlatformRequestDecoder.decode(
      line:
        #"{"version":4,"id":"capture-invalid-directory","method":"captureDisplay","params":{"delivery":"clipboard","saveDirectory":"/tmp/custom-captures"}}"#
    )
  }
}

@Test
func decodesPrepareRegionRequest() throws {
  let request = try PlatformRequestDecoder.decode(
    line:
      #"{"version":4,"id":"prepare-1","method":"prepareRegion","params":{"targetId":"target-17"}}"#
  )

  #expect(request.method == .prepareRegion)
  #expect(request.targetId == "target-17")
}

@Test
func decodesCommitRegionRequest() throws {
  let request = try PlatformRequestDecoder.decode(
    line:
      #"{"version":4,"id":"commit-1","method":"commitRegion","params":{"sessionId":"region-session-17","delivery":"both","geometry":{"coordinateSpace":"target-logical","x":10.5,"y":20,"width":640,"height":360}}}"#
  )

  #expect(request.commitRegion?.sessionId == "region-session-17")
  #expect(request.commitRegion?.delivery == .both)
  #expect(request.commitRegion?.geometry.x == 10.5)
  #expect(request.commitRegion?.geometry.width == 640)
}

@Test
func decodesCancelRegionRequest() throws {
  let request = try PlatformRequestDecoder.decode(
    line:
      #"{"version":4,"id":"cancel-1","method":"cancelRegion","params":{"sessionId":"region-session-17"}}"#
  )

  #expect(request.method == .cancelRegion)
  #expect(request.sessionId == "region-session-17")
}

@Test
func rejectsUnknownFields() {
  #expect(throws: PlatformProtocolError.self) {
    try PlatformRequestDecoder.decode(
      line: #"{"version":4,"id":"bad-1","method":"getCapabilities","params":{},"extra":true}"#
    )
  }
}

@Test
func rejectsUnknownProtocolVersion() {
  #expect(throws: PlatformProtocolError.self) {
    try PlatformRequestDecoder.decode(
      line: #"{"version":2,"id":"bad-2","method":"getCapabilities","params":{}}"#
    )
  }
}

@Test
func encodesCapabilitiesResponseWithoutOptionalNulls() throws {
  let response = PlatformResponse.success(
    id: "capabilities-1",
    result: .capabilities(
      PlatformCapabilities(
        contractVersion: 4,
        platform: "macos",
        hostStatus: "available",
        captureModes: [.display],
        deliveryTargets: [.folder],
        hdrCapture: "unvalidated",
        outputProfiles: ["srgb-visual-match"]
      )
    )
  )

  let line = try PlatformResponseEncoder.encodeLine(response)
  let object = try #require(
    JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any]
  )

  #expect(object["version"] as? Int == 4)
  #expect(object["id"] as? String == "capabilities-1")
  #expect(object["error"] == nil)
  let result = try #require(object["result"] as? [String: Any])
  #expect(result["platform"] as? String == "macos")
  #expect(result["activeTarget"] == nil)
}

@Test(arguments: [
  (true, CGFloat(16.0), true),
  (true, CGFloat(1.0), false),
  (false, CGFloat(16.0), false),
])
func detectsHDRFromTargetDisplayHeadroom(
  architectureSupportsHDR: Bool,
  maximumPotentialHeadroom: CGFloat,
  expected: Bool
) {
  #expect(
    DisplayDynamicRange.supportsHDR(
      architectureSupportsHDR: architectureSupportsHDR,
      maximumPotentialHeadroom: maximumPotentialHeadroom
    ) == expected
  )
}

@Test
func resolvesHiDPIDisplayPixelsFromFilterScale() throws {
  let output = try #require(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 2560, height: 1440),
      filterLogicalSize: LogicalSize(width: 2560, height: 1440),
      pointPixelScale: 2,
      region: nil
    )
  )

  #expect(output.cropRect == nil)
  #expect(output.capturePixelWidth == 5120)
  #expect(output.capturePixelHeight == 2880)
  #expect(output.pixelWidth == 5120)
  #expect(output.pixelHeight == 2880)
}

@Test(
  arguments: [
    LogicalSize(width: 5120, height: 2880),
    LogicalSize(width: 4096, height: 2304),
    LogicalSize(width: 3413.33, height: 1920),
    LogicalSize(width: 2560, height: 1440),
  ]
)
func regionPreviewUsesOnePixelPerRoundedLogicalPoint(logicalSize: LogicalSize) throws {
  let preview = try #require(RegionPreviewGeometry.pixelSize(for: logicalSize))

  #expect(preview.width == Int(logicalSize.width.rounded()))
  #expect(preview.height == Int(logicalSize.height.rounded()))
}

@Test(arguments: [false, true])
func rendersLogicalSizePreviewAsSrgbPNG(sourceIsHDR: Bool) async throws {
  let srgb = try #require(CGColorSpace(name: CGColorSpace.sRGB))
  let context = try #require(
    CGContext(
      data: nil,
      width: 4,
      height: 2,
      bitsPerComponent: 8,
      bytesPerRow: 16,
      space: srgb,
      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    )
  )
  let fillColor = try #require(
    CGColor(colorSpace: srgb, components: [0.25, 0.5, 0.75, 1])
  )
  context.setFillColor(fillColor)
  context.fill(CGRect(x: 0, y: 0, width: 4, height: 2))
  let source = try #require(context.makeImage())
  let service = MacCaptureService()

  let preview = try await service.makeVisualMatchImage(
    source,
    sourceIsHDR: sourceIsHDR,
    outputPixelSize: PixelSize(width: 2, height: 1)
  )
  let png = try await service.encodePNG(preview)
  let imageSource = try #require(CGImageSourceCreateWithData(png as CFData, nil))
  let decoded = try #require(CGImageSourceCreateImageAtIndex(imageSource, 0, nil))

  #expect(decoded.width == 2)
  #expect(decoded.height == 1)
  #expect(decoded.colorSpace?.name == CGColorSpace.sRGB)
}

@Test
func resolvesReportedBlurryRegionAtHiDPIScale() throws {
  let output = try #require(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 2560, height: 1440),
      filterLogicalSize: LogicalSize(width: 2560, height: 1440),
      pointPixelScale: 2,
      region: CaptureGeometry(
        coordinateSpace: "target-logical",
        x: 100,
        y: 50,
        width: 1408,
        height: 821
      )
    )
  )

  #expect(output.capturePixelWidth == 5120)
  #expect(output.capturePixelHeight == 2880)
  #expect(output.cropRect == CGRect(x: 200, y: 100, width: 2816, height: 1642))
  #expect(output.pixelWidth == 2816)
  #expect(output.pixelHeight == 1642)
}

@Test
func keepsOneToOneDisplaysAtLogicalSize() throws {
  let output = try #require(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 1920, height: 1080),
      filterLogicalSize: LogicalSize(width: 1920, height: 1080),
      pointPixelScale: 1,
      region: nil
    )
  )

  #expect(output.pixelWidth == 1920)
  #expect(output.pixelHeight == 1080)
}

@Test
func alignsFractionalRegionEdgesToBackingPixels() throws {
  let output = try #require(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 2560, height: 1440),
      filterLogicalSize: LogicalSize(width: 2560, height: 1440),
      pointPixelScale: 1.5,
      region: CaptureGeometry(
        coordinateSpace: "target-logical",
        x: 10.25,
        y: 20.5,
        width: 100.25,
        height: 50.25
      )
    )
  )

  #expect(output.cropRect == CGRect(x: 15, y: 30, width: 151, height: 77))
  #expect(output.pixelWidth == 151)
  #expect(output.pixelHeight == 77)
}

@Test
func alignsFractionalRegionAtTargetEdgeWithoutPadding() throws {
  let output = try #require(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 2560, height: 1440),
      filterLogicalSize: LogicalSize(width: 2560, height: 1440),
      pointPixelScale: 2,
      region: CaptureGeometry(
        coordinateSpace: "target-logical",
        x: 2500.25,
        y: 1400.25,
        width: 59.75,
        height: 39.75
      )
    )
  )

  #expect(output.cropRect == CGRect(x: 5000, y: 2800, width: 120, height: 80))
  #expect(output.pixelWidth == 120)
  #expect(output.pixelHeight == 80)
}

@Test
func alignsFractionalRegionEdgesAtOneToOneScale() throws {
  let output = try #require(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 1920, height: 1080),
      filterLogicalSize: LogicalSize(width: 1920, height: 1080),
      pointPixelScale: 1,
      region: CaptureGeometry(
        coordinateSpace: "target-logical",
        x: 10.25,
        y: 20.75,
        width: 100.5,
        height: 50.5
      )
    )
  )

  #expect(output.cropRect == CGRect(x: 10, y: 20, width: 101, height: 52))
  #expect(output.pixelWidth == 101)
  #expect(output.pixelHeight == 52)
}

@Test
func preservesRotatedDisplayAxes() throws {
  let output = try #require(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 1080, height: 1920),
      filterLogicalSize: LogicalSize(width: 1080, height: 1920),
      pointPixelScale: 2,
      region: nil
    )
  )

  #expect(output.pixelWidth == 2160)
  #expect(output.pixelHeight == 3840)
}

@Test(arguments: [0.0, -1.0, Double.nan, Double.infinity])
func rejectsInvalidPointPixelScale(pointPixelScale: Double) {
  #expect(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 2560, height: 1440),
      filterLogicalSize: LogicalSize(width: 2560, height: 1440),
      pointPixelScale: pointPixelScale,
      region: nil
    ) == nil
  )
}

@Test
func rejectsFilterTopologyMismatch() {
  #expect(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 2560, height: 1440),
      filterLogicalSize: LogicalSize(width: 1920, height: 1080),
      pointPixelScale: 2,
      region: nil
    ) == nil
  )
}

@Test
func rejectsRegionOutsideIssuedTargetBounds() {
  #expect(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 1512, height: 982),
      filterLogicalSize: LogicalSize(width: 1512, height: 982),
      pointPixelScale: 2,
      region: CaptureGeometry(
        coordinateSpace: "target-logical",
        x: 1400,
        y: 900,
        width: 200,
        height: 100
      )
    ) == nil
  )
}

@Test
func appliesPixelGeometryWithoutChangingHDRPreset() throws {
  let output = try #require(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 1728, height: 1117),
      filterLogicalSize: LogicalSize(width: 1728, height: 1117),
      pointPixelScale: 2,
      region: nil
    )
  )
  let configuration = SCStreamConfiguration(preset: .captureHDRScreenshotLocalDisplay)
  let dynamicRange = configuration.captureDynamicRange

  output.apply(to: configuration)

  #expect(configuration.width == 3456)
  #expect(configuration.height == 2234)
  #expect(configuration.captureDynamicRange == dynamicRange)
}

@Test
func capturesFullBackingFrameBeforeCroppingRegion() throws {
  let output = try #require(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 2560, height: 1440),
      filterLogicalSize: LogicalSize(width: 2560, height: 1440),
      pointPixelScale: 2,
      region: CaptureGeometry(
        coordinateSpace: "target-logical",
        x: 20,
        y: 30,
        width: 640,
        height: 360
      )
    )
  )
  let configuration = SCStreamConfiguration()
  let dynamicRange = configuration.captureDynamicRange

  output.apply(to: configuration)

  #expect(configuration.sourceRect == .zero)
  #expect(configuration.width == 5120)
  #expect(configuration.height == 2880)
  #expect(output.cropRect == CGRect(x: 40, y: 60, width: 1280, height: 720))
  #expect(configuration.captureDynamicRange == dynamicRange)
}

@Test
func cropsRegionFromFullBackingFrameWithoutResizing() throws {
  let output = try #require(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 4, height: 3),
      filterLogicalSize: LogicalSize(width: 4, height: 3),
      pointPixelScale: 2,
      region: CaptureGeometry(
        coordinateSpace: "target-logical",
        x: 0.5,
        y: 1,
        width: 2,
        height: 1.5
      )
    )
  )
  let context = try #require(
    CGContext(
      data: nil,
      width: 8,
      height: 6,
      bitsPerComponent: 8,
      bytesPerRow: 32,
      space: CGColorSpaceCreateDeviceRGB(),
      bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
    )
  )
  let fullFrame = try #require(context.makeImage())
  let cropped = try #require(output.makeOutputImage(from: fullFrame))

  #expect(output.cropRect == CGRect(x: 1, y: 2, width: 4, height: 3))
  #expect(cropped.width == 4)
  #expect(cropped.height == 3)
}

@Test
func rejectsUnexpectedFullFrameDimensions() throws {
  let output = try #require(
    CaptureOutputGeometry.resolve(
      targetLogicalSize: LogicalSize(width: 1408, height: 821),
      filterLogicalSize: LogicalSize(width: 1408, height: 821),
      pointPixelScale: 2,
      region: nil
    )
  )

  #expect(output.matchesCapture(imageWidth: 2816, imageHeight: 1642))
  #expect(!output.matchesCapture(imageWidth: 1408, imageHeight: 821))
}

@Test
func selectsPointerDisplayBeforeMainAndFallbackDisplays() {
  #expect(
    DisplayTargetSelection.selectDisplayID(
      pointerDisplayID: 17,
      mainScreenDisplayID: 23,
      fallbackDisplayID: 29
    ) == 17
  )
  #expect(
    DisplayTargetSelection.selectDisplayID(
      pointerDisplayID: nil,
      mainScreenDisplayID: 23,
      fallbackDisplayID: 29
    ) == 23
  )
  #expect(
    DisplayTargetSelection.selectDisplayID(
      pointerDisplayID: nil,
      mainScreenDisplayID: nil,
      fallbackDisplayID: 29
    ) == 29
  )
}

@Test
func encodesStructuredFailureDiagnosticWithRequestIdentity() throws {
  let line = try HostDiagnosticEncoder.encodeLine(
    HostDiagnostic(
      requestID: "capture-17",
      event: "capture-failed",
      failure: HostFailure(
        code: .unexpectedFailure,
        message: "Capture failed (ScreenCaptureKit:-1).",
        retryable: true
      )
    )
  )
  let object = try #require(
    JSONSerialization.jsonObject(with: Data(line.utf8)) as? [String: Any]
  )

  #expect(object["level"] as? String == "error")
  #expect(object["event"] as? String == "capture-failed")
  #expect(object["requestID"] as? String == "capture-17")
  #expect(object["code"] as? String == "unexpected-failure")
  #expect(object["retryable"] as? Bool == true)
}

@Test
func resolvesExplicitScreenRecordingPermissionPaths() {
  var requestCount = 0
  let alreadyGranted = ScreenRecordingPermission.resolve(preflightGranted: true) {
    requestCount += 1
    return false
  }
  #expect(alreadyGranted == .granted)
  #expect(requestCount == 0)

  let grantedAfterRequest = ScreenRecordingPermission.resolve(preflightGranted: false) {
    requestCount += 1
    return true
  }
  #expect(grantedAfterRequest == .granted)
  #expect(requestCount == 1)

  let deniedOrRestricted = ScreenRecordingPermission.resolve(preflightGranted: false) {
    requestCount += 1
    return false
  }
  #expect(deniedOrRestricted == .deniedOrRestricted)
  #expect(requestCount == 2)
}

@Test
func classifiesTaskAndUserStoppedErrorsAsCancellation() {
  #expect(CaptureErrorClassification.isCancellation(CancellationError()))
  #expect(
    CaptureErrorClassification.isCancellation(
      NSError(
        domain: SCStreamErrorDomain,
        code: SCStreamError.Code.userStopped.rawValue
      )
    )
  )
  #expect(
    !CaptureErrorClassification.isCancellation(
      NSError(domain: SCStreamErrorDomain, code: SCStreamError.Code.internalError.rawValue)
    )
  )
}
