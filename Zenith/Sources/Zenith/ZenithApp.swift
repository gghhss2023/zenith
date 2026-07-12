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
    private var windows: [NSWindow] = []
    private var cascadePoint = NSPoint.zero

    func applicationDidFinishLaunching(_ notification: Notification) {
        zn_init()
        setupMainMenu()

        openTerminalWindow(asTab: false)

        NSApp.setActivationPolicy(.regular)
        NSApp.activate(ignoringOtherApps: true)
    }

    private func makeTerminalWindow() -> NSWindow {
        let screenFrame = NSScreen.main?.frame ?? NSRect(x: 0, y: 0, width: 1200, height: 800)
        let windowWidth: CGFloat = 1000
        let windowHeight: CGFloat = 600
        let windowX = (screenFrame.width - windowWidth) / 2
        let windowY = (screenFrame.height - windowHeight) / 2

        let window = NSWindow(
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
        window.tabbingIdentifier = "ZenithTerminal"
        window.isReleasedWhenClosed = false

        guard let device = MTLCreateSystemDefaultDevice() else {
            fatalError("Metal is not supported on this device")
        }

        let terminalView = TerminalMetalView(frame: window.contentView!.bounds, device: device)
        terminalView.autoresizingMask = [.width, .height]
        window.contentView?.addSubview(terminalView)

        windows.append(window)
        NotificationCenter.default.addObserver(
            forName: NSWindow.willCloseNotification, object: window, queue: .main
        ) { [weak self] note in
            guard let closed = note.object as? NSWindow else { return }
            self?.windows.removeAll { $0 === closed }
        }

        return window
    }

    private func openTerminalWindow(asTab: Bool) {
        let window = makeTerminalWindow()

        if asTab, let keyWindow = NSApp.keyWindow ?? windows.dropLast().last {
            keyWindow.addTabbedWindow(window, ordered: .above)
        } else {
            cascadePoint = window.cascadeTopLeft(from: cascadePoint)
        }

        window.makeKeyAndOrderFront(nil)

        if let terminalView = window.contentView?.subviews.first as? TerminalMetalView {
            terminalView.startTerminal()
            window.makeFirstResponder(terminalView)
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }

    private func setupMainMenu() {
        let main = NSMenu()

        let appItem = NSMenuItem()
        main.addItem(appItem)
        let appMenu = NSMenu()
        appItem.submenu = appMenu
        appMenu.addItem(NSMenuItem(
            title: "About Zenith",
            action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: ""))
        appMenu.addItem(.separator())
        let servicesItem = NSMenuItem(title: "Services", action: nil, keyEquivalent: "")
        let servicesMenu = NSMenu(title: "Services")
        servicesItem.submenu = servicesMenu
        appMenu.addItem(servicesItem)
        appMenu.addItem(.separator())
        appMenu.addItem(NSMenuItem(
            title: "Hide Zenith", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h"))
        let hideOthers = NSMenuItem(
            title: "Hide Others",
            action: #selector(NSApplication.hideOtherApplications(_:)), keyEquivalent: "h")
        hideOthers.keyEquivalentModifierMask = [.command, .option]
        appMenu.addItem(hideOthers)
        appMenu.addItem(NSMenuItem(
            title: "Show All",
            action: #selector(NSApplication.unhideAllApplications(_:)), keyEquivalent: ""))
        appMenu.addItem(.separator())
        appMenu.addItem(NSMenuItem(
            title: "Quit Zenith", action: #selector(NSApplication.terminate(_:)), keyEquivalent: "q"))

        let shellItem = NSMenuItem()
        main.addItem(shellItem)
        let shellMenu = NSMenu(title: "Shell")
        shellItem.submenu = shellMenu
        shellMenu.addItem(NSMenuItem(
            title: "New Window", action: #selector(AppDelegate.newWindow(_:)), keyEquivalent: "n"))
        shellMenu.addItem(NSMenuItem(
            title: "New Tab", action: #selector(AppDelegate.newTab(_:)), keyEquivalent: "t"))
        shellMenu.addItem(NSMenuItem(
            title: "Close Window", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w"))

        let editItem = NSMenuItem()
        main.addItem(editItem)
        let editMenu = NSMenu(title: "Edit")
        editItem.submenu = editMenu
        editMenu.addItem(NSMenuItem(
            title: "Copy", action: #selector(TerminalMetalView.copy(_:)), keyEquivalent: "c"))
        editMenu.addItem(NSMenuItem(
            title: "Paste", action: #selector(TerminalMetalView.paste(_:)), keyEquivalent: "v"))
        editMenu.addItem(.separator())
        editMenu.addItem(NSMenuItem(
            title: "Select All", action: #selector(NSResponder.selectAll(_:)), keyEquivalent: "a"))

        let viewItem = NSMenuItem()
        main.addItem(viewItem)
        let viewMenu = NSMenu(title: "View")
        viewItem.submenu = viewMenu
        viewMenu.addItem(NSMenuItem(
            title: "AI Panel", action: #selector(TerminalMetalView.toggleAI(_:)), keyEquivalent: "k"))
        viewMenu.addItem(.separator())
        viewMenu.addItem(NSMenuItem(
            title: "Increase Font Size",
            action: #selector(TerminalMetalView.increaseFontSize(_:)), keyEquivalent: "+"))
        viewMenu.addItem(NSMenuItem(
            title: "Decrease Font Size",
            action: #selector(TerminalMetalView.decreaseFontSize(_:)), keyEquivalent: "-"))
        viewMenu.addItem(NSMenuItem(
            title: "Reset Font Size",
            action: #selector(TerminalMetalView.resetFontSize(_:)), keyEquivalent: "0"))
        viewMenu.addItem(.separator())
        let fullScreen = NSMenuItem(
            title: "Enter Full Screen",
            action: #selector(NSWindow.toggleFullScreen(_:)), keyEquivalent: "f")
        fullScreen.keyEquivalentModifierMask = [.command, .control]
        viewMenu.addItem(fullScreen)

        let windowItem = NSMenuItem()
        main.addItem(windowItem)
        let windowMenu = NSMenu(title: "Window")
        windowItem.submenu = windowMenu
        windowMenu.addItem(NSMenuItem(
            title: "Minimize", action: #selector(NSWindow.performMiniaturize(_:)), keyEquivalent: "m"))
        windowMenu.addItem(NSMenuItem(
            title: "Zoom", action: #selector(NSWindow.performZoom(_:)), keyEquivalent: ""))
        windowMenu.addItem(.separator())
        windowMenu.addItem(NSMenuItem(
            title: "Bring All to Front",
            action: #selector(NSApplication.arrangeInFront(_:)), keyEquivalent: ""))

        let helpItem = NSMenuItem()
        main.addItem(helpItem)
        let helpMenu = NSMenu(title: "Help")
        helpItem.submenu = helpMenu
        helpMenu.addItem(NSMenuItem(
            title: "Zenith on GitHub",
            action: #selector(AppDelegate.openGitHub(_:)), keyEquivalent: ""))

        NSApp.mainMenu = main
        NSApp.servicesMenu = servicesMenu
        NSApp.windowsMenu = windowMenu
        NSApp.helpMenu = helpMenu
    }

    @objc func newWindow(_ sender: Any?) {
        openTerminalWindow(asTab: false)
    }

    @objc func newTab(_ sender: Any?) {
        openTerminalWindow(asTab: true)
    }

    @objc func openGitHub(_ sender: Any?) {
        NSWorkspace.shared.open(URL(string: "https://github.com/Gkyohd/zenith")!)
    }
}
