// Renders Devid's icons from the logo's SVG geometry (D-026) with CoreGraphics.
// Run from this folder: `swiftc -target x86_64-apple-macos14.0 render.swift -o /tmp/render && /tmp/render`.
// (`swift render.swift` fails here: its JIT misses CoreGraphics symbols when the SDK is newer
// than the OS.) Writes icon.png and tray.png.
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

// App icon, as on the "Ukuran ikon macOS" board: a 824/1024 plate, inset 100.
render("icon.png", w: 1024, h: 1024, view: CGRect(x: -100 / 8.24, y: -100 / 8.24, width: 1024 / 8.24, height: 1024 / 8.24)) { c in
    let ink = color(0x16151F)
    c.setFillColor(ink)
    c.addPath(CGPath(roundedRect: CGRect(x: 0, y: 0, width: 100, height: 100), cornerWidth: 22.45, cornerHeight: 22.45, transform: nil))
    c.fillPath()
    c.setStrokeColor(color(0x34C77B))
    c.setLineWidth(8)
    shackle(c)
    c.strokePath()
    c.setFillColor(color(0xF4F4FA))
    c.addPath(CGPath(roundedRect: CGRect(x: 25, y: 44, width: 50, height: 36), cornerWidth: 9, cornerHeight: 9, transform: nil))
    c.fillPath()
    stroke(c, 5.5, ink, [CGPoint(x: 37, y: 55), CGPoint(x: 45, y: 62), CGPoint(x: 37, y: 69)])
    stroke(c, 5.5, ink, [CGPoint(x: 51, y: 69), CGPoint(x: 62, y: 69)])
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
