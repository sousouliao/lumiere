import AppKit
import Testing

@testable import LumiereMacHostCore

@Test
func nativeSelectionKeepsLogicalBoundsAndFullBackingPixelCrop() throws {
  let targetSize = LogicalSize(width: 2560, height: 1440)
  let viewSize = NSSize(width: 2560, height: 1440)
  let selected = try #require(NativeRegionSelectionGeometry.project(
    rect: NSRect(x: 120, y: 80, width: 300, height: 150),
    viewSize: viewSize,
    logicalSize: targetSize
  ))
  #expect(selected.x == 120)
  #expect(selected.y == 80)
  #expect(selected.width == 300)
  #expect(selected.height == 150)

  let output = try #require(CaptureOutputGeometry.resolve(
    targetLogicalSize: targetSize,
    filterLogicalSize: targetSize,
    pointPixelScale: 2,
    region: selected
  ))
  #expect(output.cropRect == CGRect(x: 240, y: 160, width: 600, height: 300))
  #expect(output.pixelWidth == 600)
  #expect(output.pixelHeight == 300)

  #expect(NativeRegionSelectionGeometry.project(
    rect: NSRect(x: 120, y: 80, width: 31, height: 24),
    viewSize: viewSize,
    logicalSize: targetSize
  ) == nil)
  #expect(NativeRegionSelectionGeometry.project(
    rect: NSRect(x: 120, y: 80, width: 32, height: 24),
    viewSize: viewSize,
    logicalSize: targetSize
  ) != nil)
}
