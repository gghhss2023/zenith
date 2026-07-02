import AppKit

final class AIPanel: NSPanel {
    enum Mode: Int {
        case command = 0
        case diagnose = 1
        case ask = 2
    }

    private let modeControl = NSSegmentedControl(
        labels: ["生成命令", "诊断错误", "问答"],
        trackingMode: .selectOne,
        target: nil,
        action: nil
    )
    private let inputField = NSTextField()
    private let scrollView: NSScrollView
    private let resultView: NSTextView
    private let insertButton = NSButton(title: "插入命令", target: nil, action: nil)
    private let bridge = ClaudeBridge()
    private weak var terminalView: TerminalMetalView?
    private let aiModel: String
    private var responseText = ""

    init(terminalView: TerminalMetalView, model: String) {
        self.terminalView = terminalView
        self.aiModel = model
        self.scrollView = NSTextView.scrollableTextView()
        self.resultView = scrollView.documentView as! NSTextView

        super.init(
            contentRect: NSRect(x: 0, y: 0, width: 560, height: 320),
            styleMask: [.titled, .closable, .utilityWindow],
            backing: .buffered,
            defer: false
        )
        title = "Zenith AI"
        level = .floating
        isReleasedWhenClosed = false
        hidesOnDeactivate = false

        modeControl.selectedSegment = 0
        modeControl.target = self
        modeControl.action = #selector(modeChanged)

        inputField.placeholderString = "描述你想做什么，回车发送（诊断模式可留空）"
        inputField.target = self
        inputField.action = #selector(send)

        resultView.isEditable = false
        resultView.font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)

        insertButton.target = self
        insertButton.action = #selector(insertCommand)
        insertButton.isEnabled = false

        let stack = NSStackView(views: [modeControl, inputField, scrollView, insertButton])
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 8
        stack.edgeInsets = NSEdgeInsets(top: 12, left: 12, bottom: 12, right: 12)
        stack.translatesAutoresizingMaskIntoConstraints = false
        contentView = NSView()
        contentView!.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.topAnchor.constraint(equalTo: contentView!.topAnchor),
            stack.bottomAnchor.constraint(equalTo: contentView!.bottomAnchor),
            stack.leadingAnchor.constraint(equalTo: contentView!.leadingAnchor),
            stack.trailingAnchor.constraint(equalTo: contentView!.trailingAnchor),
            inputField.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -24),
            scrollView.widthAnchor.constraint(equalTo: stack.widthAnchor, constant: -24),
            scrollView.heightAnchor.constraint(greaterThanOrEqualToConstant: 160),
        ])
    }

    override var canBecomeKey: Bool { true }

    override func cancelOperation(_ sender: Any?) {
        bridge.cancel()
        orderOut(nil)
    }

    func toggle(over window: NSWindow?) {
        if isVisible {
            orderOut(nil)
            return
        }
        if let parent = window {
            let pf = parent.frame
            let x = pf.midX - frame.width / 2
            let y = pf.maxY - frame.height - 60
            setFrameOrigin(NSPoint(x: x, y: y))
        }
        makeKeyAndOrderFront(nil)
        makeFirstResponder(inputField)
    }

    private var mode: Mode {
        Mode(rawValue: modeControl.selectedSegment) ?? .command
    }

    @objc private func modeChanged() {
        insertButton.isEnabled = false
        inputField.placeholderString = mode == .diagnose
            ? "可留空，直接回车让 AI 分析屏幕上的报错"
            : "描述你想做什么，回车发送"
    }

    @objc private func send() {
        let input = inputField.stringValue.trimmingCharacters(in: .whitespaces)
        if input.isEmpty && mode != .diagnose { return }

        responseText = ""
        resultView.string = ""
        insertButton.isEnabled = false

        let context = terminalView?.screenText() ?? ""
        let prompt = buildPrompt(mode: mode, input: input, context: context)
        let currentMode = mode

        bridge.query(prompt: prompt, model: aiModel, onDelta: { [weak self] delta in
            guard let self = self else { return }
            self.responseText += delta
            self.resultView.string = self.responseText
            self.resultView.scrollToEndOfDocument(nil)
        }, onDone: { [weak self] result in
            guard let self = self else { return }
            switch result {
            case .success:
                switch currentMode {
                case .command:
                    self.insertButton.isEnabled = true
                case .diagnose:
                    self.insertButton.isEnabled = self.extractFixCommand() != nil
                case .ask:
                    self.insertButton.isEnabled = false
                }
            case .failure(let error):
                self.resultView.string = "⚠️ \(error.localizedDescription)"
            }
        })
    }

    @objc private func insertCommand() {
        let command: String
        switch mode {
        case .command:
            command = responseText.trimmingCharacters(in: .whitespacesAndNewlines)
        case .diagnose:
            guard let fix = extractFixCommand() else { return }
            command = fix
        case .ask:
            return
        }
        terminalView?.insertToPrompt(command)
        orderOut(nil)
    }

    private func extractFixCommand() -> String? {
        for line in responseText.components(separatedBy: "\n").reversed() {
            if line.hasPrefix("FIX: ") {
                let cmd = String(line.dropFirst(5)).trimmingCharacters(in: .whitespaces)
                return cmd.isEmpty ? nil : cmd
            }
        }
        return nil
    }

    private func buildPrompt(mode: Mode, input: String, context: String) -> String {
        switch mode {
        case .command:
            return """
            You are a terminal assistant on macOS. Convert the request into a single \
            shell command. Output ONLY the command itself - no explanation, no markdown fences.

            Terminal screen (context):
            \(context)

            Request: \(input)
            """
        case .diagnose:
            return """
            You are a terminal assistant on macOS. The terminal screen below likely contains \
            a failed command and its error output. Explain briefly in Chinese why it failed. \
            If a fixed command exists, put it alone on the last line prefixed with "FIX: ".

            Terminal screen:
            \(context)

            Additional context from user (may be empty): \(input)
            """
        case .ask:
            return """
            You are a terminal assistant. Answer the user's question about the terminal \
            screen content below. Answer concisely in Chinese.

            Terminal screen:
            \(context)

            Question: \(input)
            """
        }
    }
}
