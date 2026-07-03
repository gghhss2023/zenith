#!/usr/bin/env swift
import AppKit

let size: CGFloat = 1024
let image = NSImage(size: NSSize(width: size, height: size))
image.lockFocus()

let inset: CGFloat = size * 0.1
let rect = NSRect(x: inset, y: inset, width: size - 2 * inset, height: size - 2 * inset)
let path = NSBezierPath(roundedRect: rect, xRadius: rect.width * 0.225, yRadius: rect.width * 0.225)
NSColor(red: 0.102, green: 0.106, blue: 0.149, alpha: 1.0).setFill()
path.fill()

let promptFont = NSFont(name: "Menlo-Bold", size: 380) ?? NSFont.boldSystemFont(ofSize: 380)
let prompt = NSAttributedString(string: "❯", attributes: [
    .font: promptFont,
    .foregroundColor: NSColor(red: 0.97, green: 0.47, blue: 0.56, alpha: 1.0),
])
let promptSize = prompt.size()
prompt.draw(at: NSPoint(x: size * 0.26, y: (size - promptSize.height) / 2))

let cursorRect = NSRect(x: size * 0.54, y: size * 0.5 - 140, width: 160, height: 280)
NSColor(red: 0.75, green: 0.79, blue: 0.96, alpha: 1.0).setFill()
cursorRect.fill()

image.unlockFocus()

guard let tiff = image.tiffRepresentation,
      let rep = NSBitmapImageRep(data: tiff),
      let srgb = rep.converting(to: .sRGB, renderingIntent: .default),
      let png = srgb.representation(using: .png, properties: [:]) else {
    fatalError("failed to render icon")
}
try! png.write(to: URL(fileURLWithPath: "dist/icon_1024.png"))
print("wrote dist/icon_1024.png")
