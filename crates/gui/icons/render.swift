// Renders Devid's icons from the logo's SVG geometry (D-026) with CoreGraphics.
// From this folder:
//   swiftc -target x86_64-apple-macos14.0 render.swift -o /tmp/render && /tmp/render
//   iconutil -c icns icon.iconset -o icon.icns && rm -r icon.iconset
// (`swift render.swift` fails here: its JIT misses CoreGraphics symbols when the SDK is newer
// than the OS.) Writes icon.png, icon.iconset/ and tray.png.
// The shapes are in the SVG's units with y pointing down, as on the logo canvas.
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

func color(_ hex: UInt32) -> CGColor {
    CGColor(srgbRed: CGFloat((hex >> 16) & 0xFF) / 255, green: CGFloat((hex >> 8) & 0xFF) / 255,
            blue: CGFloat(hex & 0xFF) / 255, alpha: 1)
}

/// Draws into a `w`×`h` pixel canvas that shows `view` (x, y, width, height in SVG units).
func render(_ name: String, w: Int, h: Int, view: CGRect, draw: (CGContext) -> Void) {
    let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                        space: CGColorSpace(name: CGColorSpace.sRGB)!,
                        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    ctx.translateBy(x: 0, y: CGFloat(h))
    ctx.scaleBy(x: CGFloat(w) / view.width, y: -CGFloat(h) / view.height)
    ctx.translateBy(x: -view.minX, y: -view.minY)
    ctx.setLineCap(.round)
    ctx.setLineJoin(.round)
    draw(ctx)
    let url = URL(fileURLWithPath: name)
    let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(dest, ctx.makeImage()!, nil)
    precondition(CGImageDestinationFinalize(dest), "could not write \(name)")
}

/// The shackle: `M37 46 V37 A13 13 0 0 1 63 37 V46`.
func shackle(_ c: CGContext) {
    c.move(to: CGPoint(x: 37, y: 46))
    c.addLine(to: CGPoint(x: 37, y: 37))
    c.addArc(center: CGPoint(x: 50, y: 37), radius: 13, startAngle: .pi, endAngle: 2 * .pi, clockwise: false)
    c.addLine(to: CGPoint(x: 63, y: 46))
}

func stroke(_ c: CGContext, _ width: CGFloat, _ rgb: CGColor, _ points: [CGPoint]) {
    c.setStrokeColor(rgb)
    c.setLineWidth(width)
    c.addLines(between: points)
    c.strokePath()
}

/// The app icon's drawings, as on the logo canvas's "Ukuran ikon macOS" board: small sizes get
/// thicker strokes (32 px) or lose the prompt (16 px), so they stay readable.
enum Size { case full, px32, px16 }

func appIcon(_ c: CGContext, _ size: Size) {
    let ink = color(0x16151F)
    c.setFillColor(ink)
    c.addPath(CGPath(roundedRect: CGRect(x: 0, y: 0, width: 100, height: 100), cornerWidth: 22.45, cornerHeight: 22.45, transform: nil))
    c.fillPath()
    c.setStrokeColor(color(0x34C77B))
    c.setLineWidth([Size.full: 8, .px32: 9, .px16: 12][size]!)
    shackle(c)
    c.strokePath()
    let body = [Size.full: CGRect(x: 25, y: 44, width: 50, height: 36),
                .px32: CGRect(x: 24, y: 44, width: 52, height: 38),
                .px16: CGRect(x: 22, y: 44, width: 56, height: 40)][size]!
    c.setFillColor(color(0xF4F4FA))
    c.addPath(CGPath(roundedRect: body, cornerWidth: 9, cornerHeight: 9, transform: nil))
    c.fillPath()
    switch size {
    case .full:
        stroke(c, 5.5, ink, [CGPoint(x: 37, y: 55), CGPoint(x: 45, y: 62), CGPoint(x: 37, y: 69)])
        stroke(c, 5.5, ink, [CGPoint(x: 51, y: 69), CGPoint(x: 62, y: 69)])
    case .px32:
        stroke(c, 7, ink, [CGPoint(x: 37, y: 56), CGPoint(x: 45, y: 63), CGPoint(x: 37, y: 70)])
    case .px16:
        break
    }
}

/// A 1024-unit canvas with the plate at 824, inset 100 (macOS's icon grid), in SVG units.
let plate = CGRect(x: -100 / 8.24, y: -100 / 8.24, width: 1024 / 8.24, height: 1024 / 8.24)

render("icon.png", w: 1024, h: 1024, view: plate) { appIcon($0, .full) }

// icon.icns, one PNG per slot (D-026): 16 pt @1x without the prompt, 16 pt @2x and 32 pt @1x
// with thicker strokes, 32 pt @2x and larger the full icon.
try? FileManager.default.createDirectory(atPath: "icon.iconset", withIntermediateDirectories: true)
let slots: [(String, Int, Size)] = [
    ("16x16", 16, .px16), ("16x16@2x", 32, .px32), ("32x32", 32, .px32), ("32x32@2x", 64, .full),
    ("128x128", 128, .full), ("128x128@2x", 256, .full), ("256x256", 256, .full),
    ("256x256@2x", 512, .full), ("512x512", 512, .full), ("512x512@2x", 1024, .full),
]
for (slot, px, size) in slots {
    render("icon.iconset/icon_\(slot).png", w: px, h: px, view: plate) { appIcon($0, size) }
}

// Menu-bar template: the prototype's outline padlock, black on transparent only, so macOS
// can tint it. tray-icon shows it 18 pt high, so 44 px is the @2x size.
render("tray.png", w: 40, h: 44, view: CGRect(x: 20, y: 18, width: 60, height: 66)) { c in
    let black = color(0x000000)
    c.setStrokeColor(black)
    c.setLineWidth(8)
    shackle(c)
    c.strokePath()
    c.setLineWidth(6)
    c.addPath(CGPath(roundedRect: CGRect(x: 26, y: 45, width: 48, height: 34), cornerWidth: 8, cornerHeight: 8, transform: nil))
    c.strokePath()
    stroke(c, 6, black, [CGPoint(x: 39, y: 56), CGPoint(x: 46, y: 62), CGPoint(x: 39, y: 68)])
    stroke(c, 6, black, [CGPoint(x: 51, y: 68), CGPoint(x: 61, y: 68)])
}
