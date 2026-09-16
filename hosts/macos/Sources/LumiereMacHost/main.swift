import AppKit
import Foundation
import LumiereMacHostCore

@main
enum LumiereMacHostMain {
  static func main() {
    let application = NSApplication.shared
    application.setActivationPolicy(.accessory)
    let timingReporter: @Sendable (CaptureTiming) -> Void = { timing in
      writeTimingDiagnostic(timing)
    }
    let service = MacCaptureService(timingReporter: timingReporter)
    let writer = ProtocolResponseWriter()

    if ProcessInfo.processInfo.environment["LUMIERE_NATIVE_OVERLAY_PROBE"] == "1" {
      Task {
        await service.prewarmNativeOverlay()
        let count = min(
          max(Int(ProcessInfo.processInfo.environment["LUMIERE_NATIVE_OVERLAY_PROBE_COUNT"] ?? "1") ?? 1, 1),
          100
        )
        let holdMilliseconds = min(
          max(Int(ProcessInfo.processInfo.environment["LUMIERE_NATIVE_OVERLAY_PROBE_HOLD_MS"] ?? "500") ?? 500, 100),
          30_000
        )
        let probeDelivery = ProcessInfo.processInfo.environment["LUMIERE_NATIVE_OVERLAY_PROBE_FOLDER"] == "1"
          ? DisplayCaptureParameters(delivery: .folder, saveDirectory: "/private/tmp/lumiere-native-region-probe")
          : DisplayCaptureParameters(delivery: .clipboard)
        for index in 0..<count {
          let requestID = "native-overlay-probe-\(index)"
          let cancellation = Task {
            try? await Task.sleep(for: .milliseconds(holdMilliseconds))
            await service.cancelNativeRegion(requestID: requestID)
          }
          let result = await service.captureRegionNatively(
            requestID: requestID,
            parameters: probeDelivery
          )
          cancellation.cancel()
          writeDiagnostic(level: "info", event: "native-overlay-probe-completed",
                          error: NSError(domain: result.deliveries?.first?.filePath
                                          ?? result.failure?.message ?? result.status, code: 0))
        }
        application.terminate(nil)
      }
      application.run()
      return
    }

    Task.detached {
      await service.prewarmNativeOverlay()
      await withTaskGroup(of: Void.self) { requests in
        while let line = readLine(strippingNewline: true) {
          let request: PlatformRequest
          do {
            request = try PlatformRequestDecoder.decode(line: line)
          } catch {
            await writer.write(invalidRequestResponse(line: line, error: error))
            continue
          }
          if request.method == .captureRegion,
            !(await service.reserveNativeRegion(requestID: request.id))
          {
            await writer.write(.success(
              id: request.id,
              result: .capture(.failed(HostFailure(
                code: .captureUnavailable,
                message: "A Region capture is already in progress.",
                retryable: true
              ))),
              version: request.version
            ))
            continue
          }
          requests.addTask {
            let response = await service.response(for: request)
            await writer.write(response)
          }
        }
        await service.shutdown()
      }
      await MainActor.run { application.terminate(nil) }
    }
    application.run()
  }

  private static func invalidRequestResponse(line: String, error: Error) -> PlatformResponse {
    let requestID = extractRequestID(from: line) ?? "invalid-request"
    writeDiagnostic(level: "warning", event: "invalid-request", error: error)
    return .failure(
      id: requestID,
      error: HostFailure(
        code: .invalidRequest,
        message: String(describing: error),
        retryable: false
      ),
      version: extractProtocolVersion(from: line) ?? platformContractVersion
    )
  }

  private actor ProtocolResponseWriter {
    func write(_ response: PlatformResponse) {
      do {
        print(try PlatformResponseEncoder.encodeLine(response))
        fflush(stdout)
        writeFailureDiagnostic(for: response)
      } catch {
        writeDiagnostic(level: "error", event: "response-encoding-failed", error: error)
      }
    }
  }

  private static func writeFailureDiagnostic(for response: PlatformResponse) {
    let failure: HostFailure?
    let event: String
    if let responseFailure = response.error {
      failure = responseFailure
      event = "request-failed"
    } else if case .capture(let captureResult)? = response.result,
      let captureFailure = captureResult.failure
    {
      failure = captureFailure
      event = "capture-failed"
    } else if case .capture(let captureResult)? = response.result,
      let deliveryFailure = captureResult.deliveries?.compactMap(\.failure).first
    {
      failure = deliveryFailure
      event = "delivery-failed"
    } else {
      return
    }

    guard let failure else {
      return
    }
    do {
      let diagnostic = HostDiagnostic(
        requestID: response.id,
        event: event,
        failure: failure
      )
      var line = try HostDiagnosticEncoder.encodeLine(diagnostic)
      line.append("\n")
      FileHandle.standardError.write(Data(line.utf8))
    } catch {
      writeDiagnostic(level: "error", event: "diagnostic-encoding-failed", error: error)
    }
  }

  private static func extractRequestID(from line: String) -> String? {
    guard let data = line.data(using: .utf8),
      let object = try? JSONSerialization.jsonObject(with: data),
      let envelope = object as? [String: Any]
    else {
      return nil
    }
    return envelope["id"] as? String
  }

  private static func extractProtocolVersion(from line: String) -> Int? {
    guard let data = line.data(using: .utf8),
      let envelope = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
      let version = envelope["version"] as? Int,
      version == platformContractVersion || version == nativeRegionContractVersion
    else { return nil }
    return version
  }

  private static func writeDiagnostic(level: String, event: String, error: Error) {
    let diagnostic: [String: String] = [
      "level": level,
      "event": event,
      "message": String(describing: error),
    ]
    guard let data = try? JSONSerialization.data(withJSONObject: diagnostic),
      var line = String(data: data, encoding: .utf8)
    else {
      return
    }
    line.append("\n")
    FileHandle.standardError.write(Data(line.utf8))
  }

  private static func writeTimingDiagnostic(_ timing: CaptureTiming) {
    var diagnostic: [String: Any] = [
      "level": "info",
      "event": "region-capture-timing",
      "requestID": timing.requestID,
      "stage": timing.stage,
      "elapsedMilliseconds": timing.elapsedMilliseconds,
      "stageMilliseconds": timing.stageMilliseconds,
    ]
    if let width = timing.width {
      diagnostic["width"] = width
    }
    if let height = timing.height {
      diagnostic["height"] = height
    }
    if let bytes = timing.bytes {
      diagnostic["bytes"] = bytes
    }
    guard
      let data = try? JSONSerialization.data(
        withJSONObject: diagnostic,
        options: [.sortedKeys]
      ),
      var line = String(data: data, encoding: .utf8)
    else {
      return
    }
    line.append("\n")
    FileHandle.standardError.write(Data(line.utf8))
  }
}
