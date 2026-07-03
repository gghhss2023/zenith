#!/usr/bin/env swift
import AppKit
import CoreText

let size: CGFloat = 1024
let pink = NSColor(red: 0.97, green: 0.47, blue: 0.56, alpha: 1.0)
let periwinkle = NSColor(red: 0.75, green: 0.79, blue: 0.96, alpha: 1.0)

func srgb(_ r: CGFloat, _ g: CGFloat, _ b: CGFloat, _ a: CGFloat = 1.0) -> CGColor {
    CGColor(colorSpace: CGColorSpace(name: CGColorSpace.sRGB)!, components: [r, g, b, a])!
}

let image = NSImage(size: NSSize(width: size, height: size))
image.lockFocus()
let ctx = NSGraphicsContext.current!.cgContext

let inset: CGFloat = size * 0.1
let rect = CGRect(x: inset, y: inset, width: size - 2 * inset, height: size - 2 * inset)
let radius = rect.width * 0.225
let path = CGPath(roundedRect: rect, cornerWidth: radius, cornerHeight: radius, transform: nil)

// drop shadow behind the window
ctx.saveGState()
ctx.setShadow(offset: CGSize(width: 0, height: -14), blur: 36, color: srgb(0, 0, 0, 0.35))
ctx.addPath(path)
ctx.setFillColor(srgb(0.08, 0.083, 0.115))
ctx.fillPath()
ctx.restoreGState()

ctx.saveGState()
ctx.addPath(path)
ctx.clip()

// screen gradient, slightly lighter at top
let space = CGColorSpace(name: CGColorSpace.sRGB)!
let screenGrad = CGGradient(colorsSpace: space, colors: [
    srgb(0.135, 0.142, 0.20), srgb(0.078, 0.081, 0.114),
] as CFArray, locations: [0, 1])!
ctx.drawLinearGradient(screenGrad,
    start: CGPoint(x: rect.midX, y: rect.maxY),
    end: CGPoint(x: rect.midX, y: rect.minY), options: [])

// titlebar
let tbHeight: CGFloat = rect.height * 0.16
let tbRect = CGRect(x: rect.minX, y: rect.maxY - tbHeight, width: rect.width, height: tbHeight)
let tbGrad = CGGradient(colorsSpace: space, colors: [
    srgb(0.235, 0.245, 0.32), srgb(0.165, 0.172, 0.235),
] as CFArray, locations: [0, 1])!
ctx.saveGState()
ctx.clip(to: tbRect)
ctx.drawLinearGradient(tbGrad,
    start: CGPoint(x: rect.midX, y: tbRect.maxY),
    end: CGPoint(x: rect.midX, y: tbRect.minY), options: [])
ctx.restoreGState()

// titlebar separator
ctx.setFillColor(srgb(0, 0, 0, 0.45))
ctx.fill(CGRect(x: rect.minX, y: tbRect.minY - 3, width: rect.width, height: 6))

// traffic lights
let lights: [CGColor] = [srgb(1.0, 0.373, 0.341), srgb(0.996, 0.737, 0.18), srgb(0.157, 0.784, 0.251)]
let lightR: CGFloat = rect.width * 0.032
for (i, c) in lights.enumerated() {
    let cx = rect.minX + rect.width * 0.085 + CGFloat(i) * lightR * 3.1
    ctx.setFillColor(c)
    ctx.fillEllipse(in: CGRect(x: cx - lightR, y: tbRect.midY - lightR, width: lightR * 2, height: lightR * 2))
}

// top inner highlight
ctx.setFillColor(srgb(1, 1, 1, 0.12))
ctx.fill(CGRect(x: rect.minX, y: rect.maxY - 4, width: rect.width, height: 4))

// prompt: pink chevron + periwinkle cursor block
let fontSize = rect.height * 0.22
let font = NSFont(name: "Menlo-Bold", size: fontSize) ?? NSFont.boldSystemFont(ofSize: fontSize)
let attr = NSAttributedString(string: "\u{276F}", attributes: [.font: font, .foregroundColor: pink])
let line = CTLineCreateWithAttributedString(attr)
ctx.textPosition = .zero
let gb = CTLineGetImageBounds(line, ctx)
let px = rect.minX + rect.width * 0.10
let lineCenterY = tbRect.minY - rect.height * 0.20
ctx.textPosition = CGPoint(x: px - gb.minX, y: lineCenterY - gb.midY)
CTLineDraw(line, ctx)

let blockH = gb.height
let blockW = blockH * 0.52
ctx.setFillColor(periwinkle.cgColor)
ctx.fill(CGRect(x: px + gb.width + rect.width * 0.045, y: lineCenterY - blockH / 2,
                width: blockW, height: blockH))

ctx.restoreGState()

// subtle outer stroke
ctx.addPath(path)
ctx.setStrokeColor(srgb(1, 1, 1, 0.07))
ctx.setLineWidth(3)
ctx.strokePath()

image.unlockFocus()

guard let tiff = image.tiffRepresentation,
      let rep = NSBitmapImageRep(data: tiff),
      let srgbRep = rep.converting(to: .sRGB, renderingIntent: .default),
      let png = srgbRep.representation(using: .png, properties: [:]) else {
    fatalError("failed to render icon")
}
try! png.write(to: URL(fileURLWithPath: "dist/icon_1024.png"))
print("wrote dist/icon_1024.png")
