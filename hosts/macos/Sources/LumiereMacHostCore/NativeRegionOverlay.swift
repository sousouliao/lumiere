import AppKit
import CoreGraphics

/// Displays a frozen, full-pixel image without encoding it for another process.
@MainActor
public final class NativeRegionOverlay {
  private var window: NativeRegionPanel?
  private var background: NSView?
  private var pointerTimer: Timer?
  private var activationTimer: Timer?
  private var screenChangeObserver: NSObjectProtocol?
  private var activeDisplayID: CGDirectDisplayID?
  private var activeDisplayFrame: NSRect?
  private var onDisplayChange: ((CGDirectDisplayID) -> Void)?
  private var onTopologyChange: (() -> Void)?

  public init() {}

  public func prewarm() {
    guard window == nil, let screen = NSScreen.main ?? NSScreen.screens.first else { return }
    let panel = NativeRegionPanel(
      contentRect: screen.frame,
      styleMask: [.borderless, .nonactivatingPanel],
      backing: .buffered,
      defer: false,
      screen: screen
    )
    panel.isOpaque = true
    panel.backgroundColor = .black
    panel.hasShadow = false
    panel.animationBehavior = .none
    panel.level = .screenSaver
    panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
    panel.acceptsMouseMovedEvents = true
    panel.appearance = NSAppearance(named: .darkAqua)
    let background = NSView(frame: NSRect(origin: .zero, size: screen.frame.size))
    background.wantsLayer = true
    panel.contentView = background
    panel.orderOut(nil)
    window = panel
    self.background = background
  }

  public func present(
    image: CGImage,
    displayID: CGDirectDisplayID,
    logicalSize: LogicalSize,
    onDisplayChange: @escaping (CGDirectDisplayID) -> Void,
    onTopologyChange: @escaping () -> Void,
    onSelection: @escaping (CaptureGeometry?) -> Void,
    onOrdered: @escaping () -> Void,
    onInteractive: @escaping () -> Void,
    onActivationFailure: @escaping (String) -> Void
  ) -> Bool {
    dismiss()
    guard let screen = NSScreen.screens.first(where: { screenDisplayID($0) == displayID }),
      screen.frame.width > 0, screen.frame.height > 0,
      image.width > 0, image.height > 0,
      abs(screen.frame.width - logicalSize.width) < 1,
      abs(screen.frame.height - logicalSize.height) < 1
    else { return false }

    prewarm()
    guard let panel = window, let background else { return false }
    panel.setFrame(screen.frame, display: false)
    background.frame = NSRect(origin: .zero, size: screen.frame.size)
    background.layer?.contents = image
    background.layer?.contentsGravity = .resize
    background.layer?.contentsScale = CGFloat(image.width) / screen.frame.width
    let selection = NativeRegionSelectionView(frame: background.bounds, logicalSize: logicalSize) {
      [weak self] geometry in
      self?.dismiss()
      onSelection(geometry)
    }
    background.addSubview(selection)
    panel.contentView = background
    panel.initialFirstResponder = selection

    activeDisplayID = displayID
    activeDisplayFrame = screen.frame
    self.onDisplayChange = onDisplayChange
    self.onTopologyChange = onTopologyChange
    panel.orderFrontRegardless()
    panel.makeKey()
    guard panel.makeFirstResponder(selection) else {
      dismiss()
      return false
    }
    onOrdered()
    var activationAttempts = 0
    activationTimer = Timer.scheduledTimer(withTimeInterval: 0.01, repeats: true) {
      [weak self, weak panel, weak selection] _ in
      MainActor.assumeIsolated {
        guard let self, let panel, let selection else { return }
        if panel.isKeyWindow && panel.firstResponder === selection {
          self.activationTimer?.invalidate()
          self.activationTimer = nil
          onInteractive()
        } else {
          activationAttempts += 1
          if activationAttempts >= 50 {
            let detail = "appActive=\(NSApp.isActive), windowKey=\(panel.isKeyWindow), responderReady=\(panel.firstResponder === selection)"
            self.dismiss()
            onActivationFailure(detail)
          }
        }
      }
    }
    pointerTimer = Timer.scheduledTimer(withTimeInterval: 1.0 / 60.0, repeats: true) {
      [weak self] _ in
      MainActor.assumeIsolated { self?.checkPointerDisplay() }
    }
    screenChangeObserver = NotificationCenter.default.addObserver(
      forName: NSApplication.didChangeScreenParametersNotification,
      object: nil,
      queue: .main
    ) { [weak self] _ in
      MainActor.assumeIsolated { self?.checkDisplayTopology() }
    }
    return true
  }

  public func dismiss() {
    pointerTimer?.invalidate()
    pointerTimer = nil
    activationTimer?.invalidate()
    activationTimer = nil
    if let screenChangeObserver { NotificationCenter.default.removeObserver(screenChangeObserver) }
    screenChangeObserver = nil
    activeDisplayID = nil
    activeDisplayFrame = nil
    onDisplayChange = nil
    onTopologyChange = nil
    window?.orderOut(nil)
    background?.subviews.forEach { $0.removeFromSuperview() }
    background?.layer?.contents = nil
  }

  public func dispose() {
    dismiss()
    window?.close()
    window = nil
    background = nil
  }

  private func checkPointerDisplay() {
    guard let activeDisplayID,
      let nextScreen = NSScreen.screens.first(where: {
        NSMouseInRect(NSEvent.mouseLocation, $0.frame, false)
      }),
      let nextDisplayID = screenDisplayID(nextScreen),
      nextDisplayID != activeDisplayID
    else { return }
    let callback = onDisplayChange
    dismiss()
    callback?(nextDisplayID)
  }

  private func checkDisplayTopology() {
    guard let activeDisplayID, let activeDisplayFrame else { return }
    guard let screen = NSScreen.screens.first(where: { screenDisplayID($0) == activeDisplayID }),
      screen.frame == activeDisplayFrame
    else {
      let callback = onTopologyChange
      dismiss()
      callback?()
      return
    }
  }

  private func screenDisplayID(_ screen: NSScreen) -> CGDirectDisplayID? {
    screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID
  }
}

private final class NativeRegionPanel: NSPanel {
  override var canBecomeKey: Bool { true }
  override var canBecomeMain: Bool { false }
}

@MainActor
private final class NativeRegionSelectionView: NSView {
  private let logicalSize: LogicalSize
  private let onSelection: (CaptureGeometry?) -> Void
  private var start: NSPoint?
  private var pointer: NSPoint?
  private var selection: NSRect?

  init(frame: NSRect, logicalSize: LogicalSize, onSelection: @escaping (CaptureGeometry?) -> Void) {
    self.logicalSize = logicalSize
    self.onSelection = onSelection
    super.init(frame: frame)
  }

  required init?(coder: NSCoder) { nil }
  override var isFlipped: Bool { true }
  override var acceptsFirstResponder: Bool { true }

  override func mouseDown(with event: NSEvent) {
    start = localPoint(event)
    pointer = start
    selection = nil
    needsDisplay = true
  }

  override func mouseDragged(with event: NSEvent) {
    pointer = localPoint(event)
    if let start, let pointer {
      selection = NSRect(
        x: min(start.x, pointer.x),
        y: min(start.y, pointer.y),
        width: abs(start.x - pointer.x),
        height: abs(start.y - pointer.y)
      )
    }
    needsDisplay = true
  }

  override func mouseMoved(with event: NSEvent) {
    pointer = localPoint(event)
    needsDisplay = true
  }

  override func mouseUp(with event: NSEvent) {
    mouseDragged(with: event)
    guard let rect = selection,
      let geometry = NativeRegionSelectionGeometry.project(
        rect: rect,
        viewSize: bounds.size,
        logicalSize: logicalSize
      )
    else {
      onSelection(nil)
      return
    }
    onSelection(geometry)
  }

  override func rightMouseDown(with event: NSEvent) { onSelection(nil) }

  override func keyDown(with event: NSEvent) {
    if event.keyCode == 53 { onSelection(nil) }
    else { super.keyDown(with: event) }
  }

  override func draw(_ dirtyRect: NSRect) {
    super.draw(dirtyRect)
    let dark = NSColor.black.withAlphaComponent(0.46)
    dark.setFill()
    if let rect = selection, rect.width > 0, rect.height > 0 {
      NSRect(x: 0, y: 0, width: bounds.width, height: rect.minY).fill()
      NSRect(x: 0, y: rect.maxY, width: bounds.width, height: bounds.height - rect.maxY).fill()
      NSRect(x: 0, y: rect.minY, width: rect.minX, height: rect.height).fill()
      NSRect(x: rect.maxX, y: rect.minY, width: bounds.width - rect.maxX, height: rect.height).fill()
      (isValid(rect) ? NSColor.white : NSColor.systemRed).setStroke()
      let outline = NSBezierPath(rect: rect)
      outline.lineWidth = 1
      outline.stroke()
      let size = "\(Int((rect.width * logicalSize.width / bounds.width).rounded())) × \(Int((rect.height * logicalSize.height / bounds.height).rounded()))" as NSString
      size.draw(
        at: NSPoint(x: min(rect.minX, max(0, bounds.width - 120)),
                    y: rect.maxY + 8 < bounds.height - 24 ? rect.maxY + 8 : max(8, rect.minY - 24)),
        withAttributes: [
          .font: NSFont.monospacedDigitSystemFont(ofSize: 12, weight: .medium),
          .foregroundColor: isValid(rect) ? NSColor.white : NSColor.systemRed,
        ]
      )
    } else {
      bounds.fill()
    }

    if let pointer {
      NSColor.white.withAlphaComponent(0.7).setStroke()
      let crosshair = NSBezierPath()
      crosshair.move(to: NSPoint(x: pointer.x, y: 0))
      crosshair.line(to: NSPoint(x: pointer.x, y: bounds.height))
      crosshair.move(to: NSPoint(x: 0, y: pointer.y))
      crosshair.line(to: NSPoint(x: bounds.width, y: pointer.y))
      crosshair.lineWidth = 0.5
      crosshair.stroke()
    }

    let hint = (selection.map { isValid($0) ? "Release to capture · Esc cancels" : "Keep dragging · Esc cancels" }
      ?? "Drag to select · Esc cancels") as NSString
    hint.draw(
      at: NSPoint(x: 20, y: 20),
      withAttributes: [
        .font: NSFont.systemFont(ofSize: 12, weight: .medium),
        .foregroundColor: NSColor.white,
      ]
    )
  }

  private func localPoint(_ event: NSEvent) -> NSPoint {
    let point = convert(event.locationInWindow, from: nil)
    return NSPoint(
      x: min(max(point.x, 0), bounds.width),
      y: min(max(point.y, 0), bounds.height)
    )
  }

  private func isValid(_ rect: NSRect) -> Bool {
    NativeRegionSelectionGeometry.project(
      rect: rect, viewSize: bounds.size, logicalSize: logicalSize
    ) != nil
  }
}

enum NativeRegionSelectionGeometry {
  static func project(
    rect: NSRect,
    viewSize: NSSize,
    logicalSize: LogicalSize
  ) -> CaptureGeometry? {
    guard viewSize.width > 0, viewSize.height > 0,
      logicalSize.width > 0, logicalSize.height > 0,
      rect.minX >= 0, rect.minY >= 0,
      rect.maxX <= viewSize.width, rect.maxY <= viewSize.height
    else { return nil }
    let width = Double(rect.width / viewSize.width) * logicalSize.width
    let height = Double(rect.height / viewSize.height) * logicalSize.height
    guard width >= 32, height >= 24 else { return nil }
    return CaptureGeometry(
      coordinateSpace: "target-logical",
      x: Double(rect.minX / viewSize.width) * logicalSize.width,
      y: Double(rect.minY / viewSize.height) * logicalSize.height,
      width: width,
      height: height
    )
  }
}
