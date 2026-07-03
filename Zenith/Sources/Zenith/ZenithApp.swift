import AppKit
import MetalKit
import CZenith

@main
struct ZenithApp {
    static func main() {
        FileManager.default.changeCurrentDirectoryPath(NSHomeDirectory())
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
    }
}

class AppDelegate: NSObject, NSApplicationDelegate {
    var window: NSWindow!
    var terminalView: TerminalMetalView!

    func applicationDidFinishLaunching(_ notification: Notification) {
        zn_init()

        let screenFrame = NSScreen.main?.frame ?? NSRect(x: 0, y: 0, width: 1200, height: 800)
        let windowWidth: CGFloat = 1000
        let windowHeight: CGFloat = 600
        let windowX = (screenFrame.width - windowWidth) / 2
        let windowY = (screenFrame.height - windowHeight) / 2

        window = NSWindow(
            contentRect: NSRect(x: windowX, y: windowY, width: windowWidth, height: windowHeight),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )

        window.title = "Zenith"
        window.backgroundColor = NSColor(red: 0.102, green: 0.106, blue: 0.149, alpha: 1.0)
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        window.isOpaque = false
        window.minSize = NSSize(width: 400, height: 300)

        guard let device = MTLCreateSystemDefaultDevice() else {
            fatalError("Metal is not supported on this device")
        }

        terminalView = TerminalMetalView(frame: window.contentView!.bounds, device: device)
        terminalView.autoresizingMask = [.width, .height]

        window.contentView?.addSubview(terminalView)
        window.makeKeyAndOrderFront(nil)

        terminalView.startTerminal()

        window.makeFirstResponder(terminalView)

        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }
}
