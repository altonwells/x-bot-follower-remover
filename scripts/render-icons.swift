// Render a geometric R mark at native Chrome toolbar and extension sizes.
// swift scripts/render-icons.swift extension/icons
import AppKit
import Foundation
let output = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
for size in [16, 32, 48, 128] {
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: size, pixelsHigh: size,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)!
    let scale = CGFloat(size) / 128
    let transform = NSAffineTransform(); transform.scale(by: scale); transform.concat()
    NSColor(srgbRed: 0.14, green: 0.31, blue: 0.24, alpha: 1).setFill()
    NSBezierPath(roundedRect: NSRect(x: 0, y: 0, width: 128, height: 128), xRadius: 28, yRadius: 28).fill()
    let glyph = NSBezierPath()
    glyph.move(to: NSPoint(x: 35, y: 26)); glyph.line(to: NSPoint(x: 35, y: 102))
    glyph.line(to: NSPoint(x: 67, y: 102))
    glyph.curve(to: NSPoint(x: 94, y: 78), controlPoint1: NSPoint(x: 85, y: 102), controlPoint2: NSPoint(x: 94, y: 94))
    glyph.curve(to: NSPoint(x: 77, y: 56), controlPoint1: NSPoint(x: 94, y: 66), controlPoint2: NSPoint(x: 89, y: 59))
    glyph.line(to: NSPoint(x: 97, y: 26)); glyph.line(to: NSPoint(x: 77, y: 26))
    glyph.line(to: NSPoint(x: 60, y: 53)); glyph.line(to: NSPoint(x: 52, y: 53))
    glyph.line(to: NSPoint(x: 52, y: 26)); glyph.close()
    glyph.move(to: NSPoint(x: 52, y: 69)); glyph.line(to: NSPoint(x: 65, y: 69))
    glyph.curve(to: NSPoint(x: 77, y: 78), controlPoint1: NSPoint(x: 73, y: 69), controlPoint2: NSPoint(x: 77, y: 72))
    glyph.curve(to: NSPoint(x: 65, y: 87), controlPoint1: NSPoint(x: 77, y: 84), controlPoint2: NSPoint(x: 73, y: 87))
    glyph.line(to: NSPoint(x: 52, y: 87)); glyph.close()
    glyph.windingRule = .evenOdd
    NSColor(srgbRed: 0.90, green: 0.96, blue: 0.85, alpha: 1).setFill(); glyph.fill()
    NSGraphicsContext.restoreGraphicsState()
    try bitmap.representation(using: .png, properties: [:])!.write(to: output.appendingPathComponent("r-\(size).png"))
}
