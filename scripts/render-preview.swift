// macOS-only, offscreen renderer for examples/preview.rs. No app or window is opened.
// swift scripts/render-preview.swift /tmp/forgive-me-preview.json docs/previews
import AppKit
import Foundation

let input = URL(fileURLWithPath: CommandLine.arguments[1])
let output = URL(fileURLWithPath: CommandLine.arguments[2], isDirectory: true)
try FileManager.default.createDirectory(at: output, withIntermediateDirectories: true)
let scenes = try JSONSerialization.jsonObject(with: Data(contentsOf: input)) as! [[String: Any]]
func color(_ hex: String) -> NSColor {
    let value = UInt32(hex.dropFirst(), radix: 16)!
    return NSColor(srgbRed: CGFloat((value >> 16) & 255) / 255,
                   green: CGFloat((value >> 8) & 255) / 255,
                   blue: CGFloat(value & 255) / 255, alpha: 1)
}
for scene in scenes {
    let columns = scene["width"] as! Int, rows = scene["height"] as! Int
    let cw = 10, ch = 21, width = columns * cw, height = rows * ch
    let bitmap = NSBitmapImageRep(bitmapDataPlanes: nil, pixelsWide: width, pixelsHigh: height,
        bitsPerSample: 8, samplesPerPixel: 4, hasAlpha: true, isPlanar: false,
        colorSpaceName: .deviceRGB, bytesPerRow: 0, bitsPerPixel: 0)!
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(bitmapImageRep: bitmap)!
    for (index, cell) in (scene["cells"] as! [[String: Any]]).enumerated() {
        let x = (index % columns) * cw, y = height - ((index / columns) + 1) * ch
        color(cell["bg"] as! String).setFill()
        NSRect(x: x, y: y, width: cw, height: ch).fill()
    }
    for (index, cell) in (scene["cells"] as! [[String: Any]]).enumerated() {
        let x = (index % columns) * cw, y = height - ((index / columns) + 1) * ch
        let font = NSFont(name: cell["bold"] as! Bool ? "Menlo-Bold" : "Menlo-Regular", size: 16)!
        (cell["text"] as! NSString).draw(at: NSPoint(x: x, y: y + 1), withAttributes: [
            .font: font, .foregroundColor: color(cell["fg"] as! String),
        ])
    }
    NSGraphicsContext.restoreGraphicsState()
    let path = output.appendingPathComponent("\(scene["name"] as! String).png")
    try bitmap.representation(using: .png, properties: [:])!.write(to: path)
    print(path.path)
}
