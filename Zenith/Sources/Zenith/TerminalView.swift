import MetalKit
import AppKit
import CZenith

class TerminalMetalView: MTKView {
    var terminal: OpaquePointer?
    var commandQueue: MTLCommandQueue!
    var bgPipeline: MTLRenderPipelineState!
    var glyphPipeline: MTLRenderPipelineState!
    var atlasTexture: MTLTexture?
    var uniformBuffer: MTLBuffer!
    var readSource: DispatchSourceRead?
    var cellWidth: Float = 8.4
    var cellHeight: Float = 16.8
    private var baseFontSize: Float = 13
    private var currentFontSize: Float = 13
    private var scrollAccumulator: CGFloat = 0
    private var selectionStart: (col: Int, row: Int)?
    private var selectionEnd: (col: Int, row: Int)?
    private var markedText: String = ""
    private var lastCursorRect: NSRect = .zero
    private var aiPanel: AIPanel?
    private var aiModel = "sonnet"

    override init(frame: CGRect, device: MTLDevice?) {
        super.init(frame: frame, device: device ?? MTLCreateSystemDefaultDevice())
        commonInit()
    }

    required init(coder: NSCoder) {
        super.init(coder: coder)
        self.device = MTLCreateSystemDefaultDevice()
        commonInit()
    }

    private func commonInit() {
        guard let device = self.device else { return }

        self.commandQueue = device.makeCommandQueue()
        self.colorPixelFormat = .bgra8Unorm
        self.clearColor = MTLClearColor(red: 0.102, green: 0.106, blue: 0.149, alpha: 1.0)
        // Draw on demand only. Continuous drawing floods the main queue with
        // draw callbacks whenever the drawable pool stalls (each blocks ~1s in
        // nextDrawable), starving keyboard/PTY event processing indefinitely.
        self.isPaused = true
        self.enableSetNeedsDisplay = true

        let library: MTLLibrary
        do {
            library = try device.makeLibrary(source: metalShaderSource, options: nil)
        } catch {
            fatalError("Failed to compile Metal shaders: \(error)")
        }

        let bgDesc = MTLRenderPipelineDescriptor()
        bgDesc.vertexFunction = library.makeFunction(name: "bg_vertex")
        bgDesc.fragmentFunction = library.makeFunction(name: "bg_fragment")
        bgDesc.colorAttachments[0].pixelFormat = self.colorPixelFormat
        bgDesc.colorAttachments[0].isBlendingEnabled = true
        bgDesc.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        bgDesc.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        bgDesc.colorAttachments[0].sourceAlphaBlendFactor = .sourceAlpha
        bgDesc.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        bgPipeline = try! device.makeRenderPipelineState(descriptor: bgDesc)

        let glyphDesc = MTLRenderPipelineDescriptor()
        glyphDesc.vertexFunction = library.makeFunction(name: "glyph_vertex")
        glyphDesc.fragmentFunction = library.makeFunction(name: "glyph_fragment")
        glyphDesc.colorAttachments[0].pixelFormat = self.colorPixelFormat
        glyphDesc.colorAttachments[0].isBlendingEnabled = true
        glyphDesc.colorAttachments[0].sourceRGBBlendFactor = .sourceAlpha
        glyphDesc.colorAttachments[0].destinationRGBBlendFactor = .oneMinusSourceAlpha
        glyphDesc.colorAttachments[0].sourceAlphaBlendFactor = .sourceAlpha
        glyphDesc.colorAttachments[0].destinationAlphaBlendFactor = .oneMinusSourceAlpha
        glyphPipeline = try! device.makeRenderPipelineState(descriptor: glyphDesc)

        uniformBuffer = device.makeBuffer(length: 8, options: .storageModeShared)
    }

    func startTerminal() {
        let scale = Float(self.window?.backingScaleFactor ?? 2.0)
        let cols = UInt32(max(Float(bounds.width) * scale / cellWidth, 80))
        let rows = UInt32(max(Float(bounds.height) * scale / cellHeight, 24))

        let config = zn_config_load()!
        terminal = zn_terminal_new(cols, rows, config.pointee.font_family, config.pointee.font_size * scale)
        baseFontSize = config.pointee.font_size
        currentFontSize = baseFontSize
        aiModel = String(cString: config.pointee.ai_model)
        zn_config_free(config)

        guard let terminal = terminal else {
            fatalError("Failed to create terminal")
        }

        var w: Float = 0
        var h: Float = 0
        zn_terminal_cell_size(terminal, &w, &h)
        cellWidth = w
        cellHeight = h

        let fd = zn_terminal_pty_fd(terminal)
        let source = DispatchSource.makeReadSource(fileDescriptor: fd, queue: .main)
        source.setEventHandler { [weak self] in
            guard let self = self, let term = self.terminal else { return }
            while zn_terminal_read(term) {}
            self.needsDisplay = true
            if zn_terminal_child_exited(term) >= 0 {
                self.readSource?.cancel()
                self.window?.close()
            }
        }
        source.resume()
        self.readSource = source
    }

    deinit {
        readSource?.cancel()
        if let terminal = terminal {
            zn_terminal_destroy(terminal)
        }
    }

    override var acceptsFirstResponder: Bool { true }

    private func cellAt(_ event: NSEvent) -> (col: Int, row: Int) {
        let loc = convert(event.locationInWindow, from: nil)
        let scale = CGFloat(self.window?.backingScaleFactor ?? 2.0)
        let col = Int((loc.x * scale) / CGFloat(cellWidth))
        let row = Int(((bounds.height - loc.y) * scale) / CGFloat(cellHeight))
        return (max(col, 0), max(row, 0))
    }

    private func clearSelection() {
        if selectionStart != nil { needsDisplay = true }
        selectionStart = nil
        selectionEnd = nil
    }

    private func normalizedSelection() -> (start: (col: Int, row: Int), end: (col: Int, row: Int))? {
        guard let s = selectionStart, let e = selectionEnd else { return nil }
        if s.row == e.row && s.col == e.col { return nil }
        if (e.row, e.col) < (s.row, s.col) { return (e, s) }
        return (s, e)
    }

    override func mouseDown(with event: NSEvent) {
        let cell = cellAt(event)
        selectionStart = cell
        selectionEnd = cell
        needsDisplay = true
    }

    override func mouseDragged(with event: NSEvent) {
        guard selectionStart != nil else { return }
        selectionEnd = cellAt(event)
        needsDisplay = true
    }

    private func copySelection() {
        guard let terminal = terminal, let sel = normalizedSelection() else { return }
        guard let cstr = zn_terminal_selection_text(
            terminal,
            UInt32(sel.start.col), UInt32(sel.start.row),
            UInt32(sel.end.col), UInt32(sel.end.row)
        ) else { return }
        let text = String(cString: cstr)
        zn_string_free(cstr)
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(text, forType: .string)
    }

    private func pasteClipboard() {
        guard let terminal = terminal,
              let text = NSPasteboard.general.string(forType: .string) else { return }
        let bytes = Array(text.replacingOccurrences(of: "\n", with: "\r").utf8)
        bytes.withUnsafeBufferPointer { buf in
            zn_terminal_write(terminal, buf.baseAddress, UInt32(buf.count))
        }
    }

    func screenText() -> String {
        guard let terminal = terminal else { return "" }
        guard let cstr = zn_terminal_screen_text(terminal, 50) else { return "" }
        let text = String(cString: cstr)
        zn_string_free(cstr)
        return text
    }

    func insertToPrompt(_ text: String) {
        guard let terminal = terminal else { return }
        // Newlines would execute intermediate lines - collapse to single line.
        let clean = text
            .replacingOccurrences(of: "\n", with: " ")
            .trimmingCharacters(in: .whitespacesAndNewlines)
        let bytes = Array(clean.utf8)
        bytes.withUnsafeBufferPointer { buf in
            zn_terminal_write(terminal, buf.baseAddress, UInt32(buf.count))
        }
    }

    private func toggleAIPanel() {
        if aiPanel == nil {
            aiPanel = AIPanel(terminalView: self, model: aiModel)
        }
        aiPanel?.toggle(over: window)
    }

    @objc func copy(_ sender: Any?) { copySelection() }
    @objc func paste(_ sender: Any?) { pasteClipboard() }
    @objc func toggleAI(_ sender: Any?) { toggleAIPanel() }

    @objc func increaseFontSize(_ sender: Any?) { setFontSize(currentFontSize + 1) }
    @objc func decreaseFontSize(_ sender: Any?) { setFontSize(currentFontSize - 1) }
    @objc func resetFontSize(_ sender: Any?) { setFontSize(baseFontSize) }

    private func setFontSize(_ size: Float) {
        guard let terminal = terminal else { return }
        let clamped = min(max(size, 6), 72)
        guard clamped != currentFontSize else { return }
        currentFontSize = clamped
        let scale = Float(self.window?.backingScaleFactor ?? 2.0)
        zn_terminal_set_font_size(terminal, clamped * scale)
        var w: Float = 0
        var h: Float = 0
        zn_terminal_cell_size(terminal, &w, &h)
        cellWidth = w
        cellHeight = h
        updateTerminalSize()
        needsDisplay = true
    }

    override func selectAll(_ sender: Any?) {
        let scale = Float(self.window?.backingScaleFactor ?? 2.0)
        let cols = max(Int(Float(bounds.width) * scale / cellWidth), 1)
        let rows = max(Int(Float(bounds.height) * scale / cellHeight), 1)
        selectionStart = (0, 0)
        selectionEnd = (cols - 1, rows - 1)
        needsDisplay = true
    }

    override func performKeyEquivalent(with event: NSEvent) -> Bool {
        guard event.modifierFlags.contains(.command) else {
            return super.performKeyEquivalent(with: event)
        }
        switch event.charactersIgnoringModifiers {
        case "c": copySelection(); return true
        case "v": pasteClipboard(); return true
        case "q": NSApp.terminate(nil); return true
        case "k": toggleAIPanel(); return true
        case "a": selectAll(nil); return true
        case "+", "=": increaseFontSize(nil); return true
        case "-": decreaseFontSize(nil); return true
        case "0": resetFontSize(nil); return true
        default: return super.performKeyEquivalent(with: event)
        }
    }

    override func keyDown(with event: NSEvent) {
        guard let terminal = terminal else { return }
        clearSelection()

        if hasMarkedText() {
            inputContext?.handleEvent(event)
            return
        }

        let modifiers = event.modifierFlags

        if event.keyCode == 124,
           !modifiers.contains(.shift), !modifiers.contains(.control),
           !modifiers.contains(.option), !modifiers.contains(.command),
           let cstr = zn_terminal_accept_suggestion(terminal) {
            let bytes = Array(String(cString: cstr).utf8)
            zn_string_free(cstr)
            bytes.withUnsafeBufferPointer { buf in
                zn_terminal_write(terminal, buf.baseAddress, UInt32(buf.count))
            }
            return
        }

        if let special = specialKeySequence(event) {
            special.withUnsafeBufferPointer { buf in
                zn_terminal_write(terminal, buf.baseAddress, UInt32(buf.count))
            }
            return
        }

        if modifiers.contains(.option) {
            if let chars = event.characters {
                for b in chars.utf8 {
                    let data: [UInt8] = [0x1b, b]
                    data.withUnsafeBufferPointer { buf in
                        zn_terminal_write(terminal, buf.baseAddress, UInt32(buf.count))
                    }
                }
            }
            return
        }

        if inputContext?.handleEvent(event) == true { return }

        if let chars = event.characters {
            let bytes = Array(chars.utf8)
            bytes.withUnsafeBufferPointer { buf in
                zn_terminal_write(terminal, buf.baseAddress, UInt32(buf.count))
            }
        }
    }

    private func specialKeySequence(_ event: NSEvent) -> [UInt8]? {
        let keyCode = event.keyCode
        let mods = event.modifierFlags

        var mod: UInt8 = 1
        if mods.contains(.shift) { mod += 1 }
        if mods.contains(.option) { mod += 2 }
        if mods.contains(.control) { mod += 4 }

        func csi(_ final: UInt8) -> [UInt8] {
            mod == 1 ? [0x1b, 0x5b, final] : [0x1b, 0x5b, 0x31, 0x3b, 0x30 + mod, final]
        }
        func csiTilde(_ num: UInt8) -> [UInt8] {
            mod == 1 ? [0x1b, 0x5b, 0x30 + num, 0x7e] : [0x1b, 0x5b, 0x30 + num, 0x3b, 0x30 + mod, 0x7e]
        }

        switch keyCode {
        case 36: return [0x0d]        // Return
        case 48: return mods.contains(.shift) ? [0x1b, 0x5b, 0x5a] : [0x09]  // Tab / Shift+Tab
        case 51: return [0x7f]        // Backspace
        case 53: return [0x1b]        // Escape
        case 123: return csi(0x44)    // Left
        case 124: return csi(0x43)    // Right
        case 125: return csi(0x42)    // Down
        case 126: return csi(0x41)    // Up
        case 115: return csi(0x48)    // Home
        case 119: return csi(0x46)    // End
        case 116: return csiTilde(5)  // Page Up
        case 121: return csiTilde(6)  // Page Down
        case 117: return csiTilde(3)  // Delete Forward
        default:
            if mods.contains(.control), let chars = event.charactersIgnoringModifiers {
                if let c = chars.unicodeScalars.first, c.value >= 0x40 && c.value < 0x80 {
                    return [UInt8(c.value & 0x1f)]
                }
            }
            return nil
        }
    }

    override func scrollWheel(with event: NSEvent) {
        guard let terminal = terminal else { return }
        clearSelection()
        if event.hasPreciseScrollingDeltas {
            let scale = self.window?.backingScaleFactor ?? 2.0
            let cellHeightPoints = CGFloat(cellHeight) / scale
            scrollAccumulator += event.scrollingDeltaY
            let lines = Int(scrollAccumulator / cellHeightPoints)
            if lines != 0 {
                scrollAccumulator -= CGFloat(lines) * cellHeightPoints
                zn_terminal_scroll_display(terminal, Int32(lines))
                needsDisplay = true
            }
        } else {
            let lines = Int32(event.scrollingDeltaY.rounded())
            if lines != 0 {
                zn_terminal_scroll_display(terminal, lines * 3)
                needsDisplay = true
            }
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let terminal = terminal,
              let device = self.device,
              let drawable = currentDrawable,
              let passDescriptor = currentRenderPassDescriptor,
              let commandBuffer = commandQueue.makeCommandBuffer()
        else { return }

        let scale = Float(self.window?.backingScaleFactor ?? 2.0)
        let viewW = Float(bounds.width) * scale
        let viewH = Float(bounds.height) * scale

        let uniformPtr = uniformBuffer.contents().assumingMemoryBound(to: Float.self)
        uniformPtr[0] = viewW
        uniformPtr[1] = viewH

        guard let renderData = zn_terminal_render(terminal, viewW, viewH) else { return }
        let rd = renderData.pointee

        if rd.atlas_dirty || atlasTexture == nil {
            let desc = MTLTextureDescriptor.texture2DDescriptor(
                pixelFormat: .rgba8Unorm,
                width: Int(rd.atlas_width),
                height: Int(rd.atlas_height),
                mipmapped: false
            )
            atlasTexture = device.makeTexture(descriptor: desc)
            atlasTexture?.replace(
                region: MTLRegionMake2D(0, 0, Int(rd.atlas_width), Int(rd.atlas_height)),
                mipmapLevel: 0,
                withBytes: rd.atlas_data,
                bytesPerRow: Int(rd.atlas_width) * 4
            )
        }

        guard let encoder = commandBuffer.makeRenderCommandEncoder(descriptor: passDescriptor) else {
            zn_render_data_free(renderData)
            return
        }

        // setVertexBytes is limited to 4KB; instance data can be far larger, so use MTLBuffers
        if rd.bg_count > 0,
           let bgBuffer = device.makeBuffer(
               bytes: rd.bg_instances,
               length: Int(rd.bg_count) * 32, // BgInstance = 2+2+4 floats = 32 bytes
               options: .storageModeShared
           ) {
            encoder.setRenderPipelineState(bgPipeline)
            encoder.setVertexBuffer(bgBuffer, offset: 0, index: 0)
            encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6, instanceCount: Int(rd.bg_count))
        }

        if let sel = normalizedSelection() {
            var quads: [Float] = []
            let totalCols = max(Int(viewW / cellWidth), 1)
            for row in sel.start.row...sel.end.row {
                let c0 = row == sel.start.row ? sel.start.col : 0
                let c1 = row == sel.end.row ? sel.end.col : totalCols - 1
                if c1 < c0 { continue }
                quads.append(contentsOf: [
                    Float(c0) * cellWidth, Float(row) * cellHeight,
                    Float(c1 - c0 + 1) * cellWidth, cellHeight,
                    0.35, 0.5, 0.85, 0.45,
                ])
            }
            if !quads.isEmpty {
                encoder.setRenderPipelineState(bgPipeline)
                encoder.setVertexBytes(quads, length: quads.count * 4, index: 0)
                encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
                encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6, instanceCount: quads.count / 8)
            }
        }

        if rd.glyph_count > 0, let atlas = atlasTexture,
           let glyphBuffer = device.makeBuffer(
               bytes: rd.glyph_instances,
               length: Int(rd.glyph_count) * 48, // GlyphInstance = 2+2+2+2+4 floats = 48 bytes
               options: .storageModeShared
           ) {
            encoder.setRenderPipelineState(glyphPipeline)
            encoder.setVertexBuffer(glyphBuffer, offset: 0, index: 0)
            encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.setFragmentTexture(atlas, index: 0)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6, instanceCount: Int(rd.glyph_count))
        }

        if rd.has_cursor {
            encoder.setRenderPipelineState(bgPipeline)
            var cursor = rd.cursor
            encoder.setVertexBytes(&cursor, length: 32, index: 0)
            encoder.setVertexBuffer(uniformBuffer, offset: 0, index: 1)
            encoder.drawPrimitives(type: .triangle, vertexStart: 0, vertexCount: 6, instanceCount: 1)

            let s = CGFloat(scale)
            lastCursorRect = NSRect(
                x: CGFloat(rd.cursor.position.0) / s,
                y: bounds.height - CGFloat(rd.cursor.position.1 + rd.cursor.size.1) / s,
                width: CGFloat(rd.cursor.size.0) / s,
                height: CGFloat(rd.cursor.size.1) / s
            )
        }

        encoder.endEncoding()
        commandBuffer.present(drawable)
        commandBuffer.commit()

        zn_render_data_free(renderData)
        zn_terminal_clear_dirty(terminal)
    }

    override func setFrameSize(_ newSize: NSSize) {
        super.setFrameSize(newSize)
        updateTerminalSize()
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        if window != nil {
            updateTerminalSize()
        }
    }

    private func updateTerminalSize() {
        guard let terminal = terminal else { return }
        let scale = Float(self.window?.backingScaleFactor ?? 2.0)
        let cols = UInt32(Float(bounds.width) * scale / cellWidth)
        let rows = UInt32(Float(bounds.height) * scale / cellHeight)
        if cols > 0 && rows > 0 {
            zn_terminal_resize(terminal, cols, rows)
        }
    }
}

extension TerminalMetalView: NSTextInputClient {
    func insertText(_ string: Any, replacementRange: NSRange) {
        markedText = ""
        guard let terminal = terminal else { return }
        let text: String
        if let s = string as? String {
            text = s
        } else if let s = string as? NSAttributedString {
            text = s.string
        } else {
            return
        }
        let bytes = Array(text.utf8)
        bytes.withUnsafeBufferPointer { buf in
            zn_terminal_write(terminal, buf.baseAddress, UInt32(buf.count))
        }
    }

    func setMarkedText(_ string: Any, selectedRange: NSRange, replacementRange: NSRange) {
        if let s = string as? String {
            markedText = s
        } else if let s = string as? NSAttributedString {
            markedText = s.string
        }
    }

    func unmarkText() {
        markedText = ""
    }

    func selectedRange() -> NSRange {
        NSRange(location: 0, length: 0)
    }

    func markedRange() -> NSRange {
        markedText.isEmpty
            ? NSRange(location: NSNotFound, length: 0)
            : NSRange(location: 0, length: markedText.utf16.count)
    }

    func hasMarkedText() -> Bool {
        !markedText.isEmpty
    }

    func attributedSubstring(forProposedRange range: NSRange, actualRange: NSRangePointer?) -> NSAttributedString? {
        nil
    }

    func validAttributesForMarkedText() -> [NSAttributedString.Key] {
        []
    }

    func firstRect(forCharacterRange range: NSRange, actualRange: NSRangePointer?) -> NSRect {
        guard let window = window else { return .zero }
        let rect = lastCursorRect == .zero
            ? NSRect(x: 0, y: bounds.height - 20, width: 10, height: 20)
            : lastCursorRect
        return window.convertToScreen(convert(rect, to: nil))
    }

    func characterIndex(for point: NSPoint) -> Int {
        0
    }
}
