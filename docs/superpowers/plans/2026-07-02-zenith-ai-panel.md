# Zenith AI Panel (Phase 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Cmd+K floating AI panel to Zenith with three modes (command generation / error diagnosis / Q&A) backed by a fully isolated `claude` CLI subprocess with streaming output.

**Architecture:** Rust side adds one FFI function (`zn_terminal_screen_text`) for screen context capture and an `[ai]` config section. Swift side adds two new files (`ClaudeBridge.swift` for subprocess + stream-json parsing, `AIPanel.swift` for the AppKit UI) and wires Cmd+K into the existing `TerminalMetalView`. AI-generated commands are only ever inserted into the prompt line, never executed.

**Tech Stack:** Rust (zenith-core/zenith-ffi/zenith-config), Swift/AppKit (NSPanel, NSSegmentedControl, NSTextView), `claude` CLI v2.1.139+ (verified flags: `--setting-sources "" --tools "" --strict-mcp-config --output-format stream-json --verbose --include-partial-messages`).

**Spec:** `docs/superpowers/specs/2026-07-02-zenith-smart-features-design.md`

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/zenith-core/src/grid.rs` | Modify | Add `screen_text()` — extract scrollback tail + visible screen as plain text |
| `crates/zenith-core/src/term.rs` | Modify | Add test for `screen_text()` (tests module at end of file) |
| `crates/zenith-ffi/src/lib.rs` | Modify | Add `zn_terminal_screen_text` FFI; add `ai_model` to `ZNConfig` |
| `crates/zenith-ffi/build.rs` | Modify | Fix stale header-path comment |
| `crates/zenith-config/src/lib.rs` | Modify | Add `[ai]` section with `model` (default `"sonnet"`) |
| `Zenith/Sources/CZenith/zenith.h` | Modify | Declare `zn_terminal_screen_text`; add `ai_model` field (manually maintained — cbindgen is disabled) |
| `Zenith/Sources/Zenith/ClaudeBridge.swift` | Create | Locate claude CLI, spawn isolated subprocess, parse stream-json, deliver text deltas |
| `Zenith/Sources/Zenith/AIPanel.swift` | Create | NSPanel UI: mode switcher, input field, streaming result view, insert button |
| `Zenith/Sources/Zenith/TerminalView.swift` | Modify | Cmd+K toggle, `screenText()`, `insertToPrompt()`, read `ai_model` from config |

---

### Task 1: `Grid::screen_text` (Rust, TDD)

**Files:**
- Modify: `crates/zenith-core/src/grid.rs` (add method after `display_text_range`, ~line 207)
- Test: `crates/zenith-core/src/term.rs` (tests module at end of file)

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` block at the end of `crates/zenith-core/src/term.rs`:

```rust
#[test]
fn screen_text_includes_scrollback_and_screen() {
    let mut term = Terminal::new(10, 2);
    term.feed(b"one\r\ntwo\r\nthree\r\nfour");
    // 2-row grid: "one" and "two" scrolled into scrollback,
    // visible screen shows "three" and "four"
    assert_eq!(term.grid().screen_text(50), "one\ntwo\nthree\nfour");
    // scrollback limited to last 1 line
    assert_eq!(term.grid().screen_text(1), "two\nthree\nfour");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p zenith-core screen_text`
Expected: FAIL with `no method named 'screen_text' found`

- [ ] **Step 3: Write minimal implementation**

Add to `impl Grid` in `crates/zenith-core/src/grid.rs`, after `display_text_range`:

```rust
pub fn screen_text(&self, scrollback_lines: usize) -> String {
    let n = scrollback_lines.min(self.scrollback.len());
    let mut lines: Vec<String> = Vec::with_capacity(n + self.rows);
    for line in &self.scrollback[self.scrollback.len() - n..] {
        let mut s = String::new();
        for cell in line {
            if cell.width != 0 {
                s.push(cell.c);
            }
        }
        lines.push(s.trim_end().to_string());
    }
    for row in 0..self.rows {
        let mut s = String::new();
        for cell in self.row_slice(row) {
            if cell.width != 0 {
                s.push(cell.c);
            }
        }
        lines.push(s.trim_end().to_string());
    }
    lines.join("\n")
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p zenith-core screen_text`
Expected: PASS (1 test)

Also run the full suite to check for regressions: `cargo test -p zenith-core`
Expected: 10 passed

- [ ] **Step 5: Commit**

```bash
git add crates/zenith-core/src/grid.rs crates/zenith-core/src/term.rs
git commit -m "feat: add Grid::screen_text for AI context capture"
```

---

### Task 2: FFI `zn_terminal_screen_text`

**Files:**
- Modify: `crates/zenith-ffi/src/lib.rs` (after `zn_terminal_selection_text`, ~line 274)
- Modify: `Zenith/Sources/CZenith/zenith.h` (after `zn_string_free` declaration, line 74)
- Modify: `crates/zenith-ffi/build.rs` (stale comment)

- [ ] **Step 1: Add FFI function**

In `crates/zenith-ffi/src/lib.rs`, after `zn_terminal_selection_text` (follow its exact pattern):

```rust
#[no_mangle]
pub extern "C" fn zn_terminal_screen_text(
    term: *mut ZenithTerminal,
    scrollback_lines: u32,
) -> *mut c_char {
    if term.is_null() {
        return std::ptr::null_mut();
    }
    let term = unsafe { &*term };
    let text = term.term.grid().screen_text(scrollback_lines as usize);
    if text.is_empty() {
        return std::ptr::null_mut();
    }
    CString::new(text).unwrap_or_default().into_raw()
}
```

- [ ] **Step 2: Declare in header**

In `Zenith/Sources/CZenith/zenith.h`, after the `zn_string_free` declaration (line 74):

```c
char *zn_terminal_screen_text(ZenithTerminal *term, uint32_t scrollback_lines);
```

- [ ] **Step 3: Fix stale comment in build.rs**

In `crates/zenith-ffi/build.rs`, change:

```rust
    // Header is maintained manually at Zenith/Sources/Zenith/zenith.h
```

to:

```rust
    // Header is maintained manually at Zenith/Sources/CZenith/zenith.h
```

- [ ] **Step 4: Verify symbol exports**

Run: `cargo build && nm -g target/debug/libzenith_ffi.dylib | grep zn_terminal_screen_text`
Expected: one line containing `_zn_terminal_screen_text`

- [ ] **Step 5: Commit**

```bash
git add crates/zenith-ffi/src/lib.rs crates/zenith-ffi/build.rs Zenith/Sources/CZenith/zenith.h
git commit -m "feat: expose zn_terminal_screen_text FFI"
```

---

### Task 3: Config `[ai] model` (Rust, TDD)

**Files:**
- Modify: `crates/zenith-config/src/lib.rs`
- Modify: `crates/zenith-ffi/src/lib.rs` (`ZNConfig`, `zn_config_load`, `zn_config_free`)
- Modify: `Zenith/Sources/CZenith/zenith.h` (`ZNConfig` struct)

- [ ] **Step 1: Write the failing test**

`crates/zenith-config/src/lib.rs` has no tests module yet. Append at the end of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ai_model_default_and_override() {
        let c: Config = toml::from_str("").unwrap();
        assert_eq!(c.ai.model, "sonnet");
        let c: Config = toml::from_str("[ai]\nmodel = \"opus\"").unwrap();
        assert_eq!(c.ai.model, "opus");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p zenith-config`
Expected: FAIL with `no field 'ai' on type 'Config'`

- [ ] **Step 3: Implement config section**

In `crates/zenith-config/src/lib.rs`:

Add `ai` field to `Config` (line 6-9):

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    pub appearance: Appearance,
    pub terminal: Terminal,
    pub ai: Ai,
}
```

Add the struct (after the `Terminal` struct, ~line 25):

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Ai {
    pub model: String,
}
```

Update `Default for Config` to include `ai: Ai::default(),` and add:

```rust
impl Default for Ai {
    fn default() -> Self {
        Self {
            model: "sonnet".into(),
        }
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p zenith-config`
Expected: PASS (1 test)

- [ ] **Step 5: Extend ZNConfig FFI**

In `crates/zenith-ffi/src/lib.rs`:

Append field to `ZNConfig` (line 246-252) — **must be last field, C layout order matters**:

```rust
#[repr(C)]
pub struct ZNConfig {
    pub font_size: f32,
    pub font_family: *const c_char,
    pub window_opacity: f32,
    pub scrollback_lines: u32,
    pub ai_model: *const c_char,
}
```

In `zn_config_load`, add before the `Box::new`:

```rust
    let ai_model = CString::new(config.ai.model.as_str()).unwrap();
```

and in the struct literal:

```rust
        ai_model: ai_model.into_raw(),
```

In `zn_config_free`, add after the `font_family` line:

```rust
            let _ = CString::from_raw(cfg.ai_model as *mut c_char);
```

- [ ] **Step 6: Mirror in header**

In `Zenith/Sources/CZenith/zenith.h`, update the `ZNConfig` typedef (lines 48-53) — same field order:

```c
typedef struct {
    float font_size;
    const char *font_family;
    float window_opacity;
    uint32_t scrollback_lines;
    const char *ai_model;
} ZNConfig;
```

- [ ] **Step 7: Verify full build**

Run: `cargo build && cargo test -p zenith-config -p zenith-core`
Expected: build OK, all tests pass

- [ ] **Step 8: Commit**

```bash
git add crates/zenith-config/src/lib.rs crates/zenith-ffi/src/lib.rs Zenith/Sources/CZenith/zenith.h
git commit -m "feat: add [ai] model config, exposed through ZNConfig"
```

---

### Task 4: ClaudeBridge.swift

**Files:**
- Create: `Zenith/Sources/Zenith/ClaudeBridge.swift`

No Swift test infrastructure exists in this project — verification is `swift build` plus the manual checklist in Task 7.

- [ ] **Step 1: Create the file with full implementation**

```swift
import Foundation

final class ClaudeBridge {
    enum BridgeError: LocalizedError {
        case cliNotFound
        case timeout
        case failed(String)

        var errorDescription: String? {
            switch self {
            case .cliNotFound:
                return "claude CLI not found (checked /usr/local/bin, /opt/homebrew/bin, ~/.local/bin)"
            case .timeout:
                return "Request timed out (60s)"
            case .failed(let msg):
                return msg
            }
        }
    }

    // GUI apps don't inherit the shell PATH, so probe known install locations.
    static func findCLI() -> String? {
        let candidates = [
            "/usr/local/bin/claude",
            "/opt/homebrew/bin/claude",
            NSHomeDirectory() + "/.local/bin/claude",
        ]
        return candidates.first { FileManager.default.isExecutableFile(atPath: $0) }
    }

    private let queue = DispatchQueue(label: "zenith.claude-bridge")
    private var process: Process?
    private var timeoutItem: DispatchWorkItem?

    func query(
        prompt: String,
        model: String,
        onDelta: @escaping (String) -> Void,
        onDone: @escaping (Result<String, BridgeError>) -> Void
    ) {
        cancel()
        guard let cli = Self.findCLI() else {
            onDone(.failure(.cliNotFound))
            return
        }

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: cli)
        // Flags verified against claude CLI v2.1.139:
        // - isolation: no user settings/hooks/plugins, no tools, no MCC servers
        // - streaming: stream-json + include-partial-messages yields text_delta events
        proc.arguments = [
            "-p", prompt,
            "--model", model,
            "--setting-sources", "",
            "--tools", "",
            "--strict-mcp-config",
            "--output-format", "stream-json",
            "--verbose",
            "--include-partial-messages",
        ]
        let stdout = Pipe()
        proc.standardOutput = stdout
        proc.standardError = Pipe()

        var buffer = Data()
        var fullText = ""
        var resultError: String?
        var didTimeout = false

        stdout.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let chunk = handle.availableData
            guard let self = self, !chunk.isEmpty else { return }
            self.queue.async {
                buffer.append(chunk)
                while let nlRange = buffer.range(of: Data([0x0A])) {
                    let lineData = buffer.subdata(in: buffer.startIndex..<nlRange.lowerBound)
                    buffer.removeSubrange(buffer.startIndex..<nlRange.upperBound)
                    guard let obj = try? JSONSerialization.jsonObject(with: lineData) as? [String: Any],
                          let type = obj["type"] as? String else { continue }
                    if type == "stream_event",
                       let event = obj["event"] as? [String: Any],
                       event["type"] as? String == "content_block_delta",
                       let delta = event["delta"] as? [String: Any],
                       delta["type"] as? String == "text_delta",
                       let text = delta["text"] as? String {
                        fullText += text
                        DispatchQueue.main.async { onDelta(text) }
                    } else if type == "result" {
                        if obj["is_error"] as? Bool == true {
                            resultError = obj["result"] as? String ?? "unknown error"
                        }
                    }
                }
            }
        }

        proc.terminationHandler = { [weak self] _ in
            guard let self = self else { return }
            self.queue.async {
                stdout.fileHandleForReading.readabilityHandler = nil
                self.timeoutItem?.cancel()
                let result: Result<String, BridgeError>
                if didTimeout {
                    result = .failure(.timeout)
                } else if let err = resultError {
                    result = .failure(.failed(err))
                } else if fullText.isEmpty {
                    result = .failure(.failed("empty response"))
                } else {
                    result = .success(fullText)
                }
                DispatchQueue.main.async { onDone(result) }
            }
        }

        let timeout = DispatchWorkItem { [weak proc] in
            didTimeout = true
            proc?.terminate()
        }
        timeoutItem = timeout
        queue.asyncAfter(deadline: .now() + 60, execute: timeout)

        do {
            try proc.run()
            process = proc
        } catch {
            onDone(.failure(.failed("failed to launch claude: \(error.localizedDescription)")))
        }
    }

    func cancel() {
        timeoutItem?.cancel()
        timeoutItem = nil
        if let proc = process, proc.isRunning {
            proc.terminationHandler = nil
            proc.terminate()
        }
        process = nil
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cd Zenith && swift build`
Expected: `Build complete!`

- [ ] **Step 3: Commit**

```bash
git add Zenith/Sources/Zenith/ClaudeBridge.swift
git commit -m "feat: add ClaudeBridge for isolated streaming claude CLI subprocess"
```

---

### Task 5: AIPanel.swift

**Files:**
- Create: `Zenith/Sources/Zenith/AIPanel.swift`

Depends on: `ClaudeBridge` (Task 4), `TerminalMetalView.screenText()` / `insertToPrompt()` (Task 6). Swift compiles per-module, so build verification happens after Task 6 — this task only creates the file and commits.

- [ ] **Step 1: Create the file with full implementation**

```swift
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
```

- [ ] **Step 2: Commit**

```bash
git add Zenith/Sources/Zenith/AIPanel.swift
git commit -m "feat: add AIPanel with three modes and streaming display"
```

---

### Task 6: Wire into TerminalMetalView

**Files:**
- Modify: `Zenith/Sources/Zenith/TerminalView.swift`

- [ ] **Step 1: Add state properties**

After `private var lastCursorRect: NSRect = .zero` (line 19):

```swift
    private var aiPanel: AIPanel?
    private var aiModel = "sonnet"
```

- [ ] **Step 2: Read ai_model from config**

In `startTerminal()`, the config is loaded at line 79-81:

```swift
        let config = zn_config_load()!
        terminal = zn_terminal_new(cols, rows, config.pointee.font_family, config.pointee.font_size * scale)
        zn_config_free(config)
```

Insert before `zn_config_free(config)`:

```swift
        aiModel = String(cString: config.pointee.ai_model)
```

- [ ] **Step 3: Add screenText / insertToPrompt / toggleAIPanel methods**

After `pasteClipboard()` (ends line 168):

```swift
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
```

- [ ] **Step 4: Route Cmd+K**

In `performKeyEquivalent` (line 170-180), add a case before `default`:

```swift
        case "k": toggleAIPanel(); return true
```

- [ ] **Step 5: Verify full build**

Run: `cargo build && cd Zenith && swift build`
Expected: both succeed

- [ ] **Step 6: Commit**

```bash
git add Zenith/Sources/Zenith/TerminalView.swift
git commit -m "feat: wire Cmd+K AI panel into terminal view"
```

---

### Task 7: Launch + manual verification

No Swift test infra — this feature is UI + subprocess, verified manually.

- [ ] **Step 1: Run all Rust tests**

Run: `cargo test`
Expected: all pass (zenith-core 10, zenith-config 1)

- [ ] **Step 2: Build and launch**

```bash
make build
pkill -f "debug/Zenith" 2>/dev/null; cd Zenith && ./.build/debug/Zenith &
```

Expected: window appears with working shell.

- [ ] **Step 3: Manual checklist (report results to user)**

1. Cmd+K → panel appears centered near top, input focused; Cmd+K again / Esc → closes
2. 生成命令 mode: type "列出当前目录最大的5个文件" → streaming text appears → result is a single command → 「插入命令」 fills prompt line, does NOT execute → panel closes → pressing Enter in terminal runs it
3. Run `cat /nonexistent_file` in terminal → Cmd+K → 诊断错误 → empty input, Enter → Chinese explanation streams in; if `FIX: ` line present, insert button enabled
4. 问答 mode: type "屏幕上最后一条命令是什么" → correct concise Chinese answer
5. Kill switch: rename claude binary temporarily (`sudo mv /usr/local/bin/claude{,.bak}`) → panel shows "claude CLI not found" error, no crash → restore (`sudo mv /usr/local/bin/claude{.bak,}`)
6. Regression: typing, selection copy/paste, Chinese IME, scrollback all still work

- [ ] **Step 4: Final commit (if any fixups were needed)**

```bash
git add -A && git commit -m "fix: AI panel polish from manual verification"
```

---

## Self-Review Notes

- Spec coverage: screen context FFI (Task 1-2), `[ai] model` config (Task 3), isolated streaming subprocess (Task 4), three-mode panel + insert-never-execute (Task 5), Cmd+K routing (Task 6), error handling for missing CLI/timeout/is_error (Task 4). Phase 2 (OSC 133 + autosuggest) is intentionally NOT in this plan — separate plan after Phase 1 ships.
- Type consistency: `screenText()`/`insertToPrompt()` defined in Task 6, referenced in Task 5 (`AIPanel` holds `weak var terminalView: TerminalMetalView?`); `ClaudeBridge.query(prompt:model:onDelta:onDone:)` matches call site in Task 5; `ZNConfig.ai_model` appended last in both Rust and C header.
- Build ordering note: Task 5's file references Task 6's methods, so `swift build` only passes after Task 6 — flagged in Task 5 header.
