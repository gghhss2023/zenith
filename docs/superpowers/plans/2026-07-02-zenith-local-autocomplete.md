# Zenith Local Autocomplete (Phase 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** OSC 133 shell integration + local command-history autosuggest, rendered as dim ghost text after the cursor, accepted with the Right-arrow key.

**Architecture:** The vte OSC dispatcher in `zenith-core` learns OSC 133 A/B/C/D markers and tracks a shell state machine (Ground/Prompt/Input/Running). At marker C the typed command is snapshotted from the grid; at marker D it is queued and the FFI layer appends it to a persistent history file (`~/.config/zenith/history`, 0600). During rendering, the FFI layer asks the terminal for the current input prefix, looks up the most-recent history entry with that prefix, and passes the remainder to `generate_render_data`, which emits dim glyphs after the cursor (display-layer only — never written to the grid). Right-arrow in Swift first tries `zn_terminal_accept_suggestion`; if a remainder exists it is written to the PTY (echoed back through the shell's own line editor), otherwise the normal CSI C sequence is sent. A shell-integration script (bash + zsh in one file) is embedded in the binary and installed idempotently to `~/.config/zenith/shell-integration.sh`; the user sources it manually (v1, no auto-inject), gated by the `ZENITH_SHELL_INTEGRATION` env var that Zenith sets on the PTY.

**Tech Stack:** Rust (vte 0.13, unicode-width 0.2), C FFI, Swift/AppKit, bash/zsh hooks (PROMPT_COMMAND/PS0, precmd/preexec).

**Spec:** `docs/superpowers/specs/2026-07-02-zenith-smart-features-design.md` — Phase 2 section. Non-goals: fuzzy matching, frequency ranking, AI involvement, auto-injection of the source line.

**Security invariant:** The accepted suggestion text is written to the PTY like typed characters. History entries can never contain `\r`/`\n` (grid extraction joins rows without newlines; file entries are single lines), so accepting a suggestion can never execute a command — the user's Enter remains the only execution gate.

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `crates/zenith-core/src/term.rs` | Modify | OSC 133 state machine, input tracking, command capture |
| `crates/zenith-core/src/history.rs` | Create | History store: load/append/dedup/suggest, 0600 file |
| `crates/zenith-core/src/shell_integration.rs` | Create | Embed + idempotently install the shell script |
| `crates/zenith-core/resources/shell-integration.sh` | Create | bash/zsh OSC 133 emitter script |
| `crates/zenith-core/src/pty.rs` | Modify | Set `ZENITH_SHELL_INTEGRATION=1`, call installer |
| `crates/zenith-core/src/lib.rs` | Modify | Register new modules |
| `crates/zenith-render/src/vertex.rs` | Modify | Ghost-text glyph emission |
| `crates/zenith-render/Cargo.toml` | Modify | Add `unicode-width` |
| `crates/zenith-ffi/src/lib.rs` | Modify | History wiring, suggestion computation, accept API |
| `Zenith/Sources/CZenith/zenith.h` | Modify | Declare `zn_terminal_accept_suggestion` |
| `Zenith/Sources/Zenith/TerminalView.swift` | Modify | Right-arrow accept hook in `keyDown` |

---

### Task 0: Create feature branch

- [ ] **Step 1: Branch off main**

```bash
cd /Users/macosx/zenith
git checkout main && git pull && git checkout -b feature/local-autocomplete
```

Expected: `Switched to a new branch 'feature/local-autocomplete'`

---

### Task 1: OSC 133 state machine in term.rs

**Files:**
- Modify: `crates/zenith-core/src/term.rs`

OSC 133 semantics: `A` = prompt start, `B` = input start (user starts typing here), `C` = pre-exec (command about to run), `D;<exit>` = command finished. `D` may arrive with no prior `C` (first prompt, empty Enter, Ctrl-C) — in that case nothing is recorded. vte splits OSC params on `;`, so `ESC ] 133 ; A BEL` arrives as `params = [b"133", b"A"]` and `133;D;0` as `[b"133", b"D", b"0"]`.

Positions are stored as `(col, abs_row)` where `abs_row = grid.scrollback_len() + cursor_row` — this stays valid when the screen scrolls (scrollback grows, screen rows shift up, absolute index is unchanged). If the input start scrolls off the top (`abs_row < scrollback_len`), extraction gives up and returns `None`.

- [ ] **Step 1: Write the failing tests**

Append to the `tests` module at the bottom of `crates/zenith-core/src/term.rs`:

```rust
    #[test]
    fn osc133_captures_completed_command() {
        let mut t = Terminal::new(40, 5);
        t.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07ls -la\r\n\x1b]133;C\x07output\r\n\x1b]133;D;0\x07");
        assert_eq!(t.take_completed_command(), Some("ls -la".to_string()));
        assert_eq!(t.take_completed_command(), None);
    }

    #[test]
    fn osc133_d_without_c_records_nothing() {
        let mut t = Terminal::new(40, 5);
        t.feed(b"\x1b]133;D;0\x07\x1b]133;A\x07$ ");
        assert_eq!(t.take_completed_command(), None);
    }

    #[test]
    fn current_input_tracks_typed_text() {
        let mut t = Terminal::new(40, 5);
        t.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07git sta");
        assert_eq!(t.current_input(), Some("git sta".to_string()));
    }

    #[test]
    fn current_input_preserves_trailing_space() {
        let mut t = Terminal::new(40, 5);
        t.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07git ");
        assert_eq!(t.current_input(), Some("git ".to_string()));
    }

    #[test]
    fn current_input_none_outside_input_state() {
        let mut t = Terminal::new(40, 5);
        assert_eq!(t.current_input(), None);
        t.feed(b"\x1b]133;A\x07$ ");
        assert_eq!(t.current_input(), None);
        t.feed(b"\x1b]133;B\x07make\r\n\x1b]133;C\x07");
        assert_eq!(t.current_input(), None); // Running state
    }

    #[test]
    fn current_input_none_when_text_right_of_cursor() {
        let mut t = Terminal::new(40, 5);
        // type "git status", then cursor-left 4 → "atus" sits right of cursor
        t.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07git status\x1b[4D");
        assert_eq!(t.current_input(), None);
    }

    #[test]
    fn osc133_command_captured_after_scroll() {
        let mut t = Terminal::new(20, 3);
        t.feed(b"x\r\ny\r\n"); // prompt lands on bottom row
        t.feed(b"\x1b]133;A\x07$ \x1b]133;B\x07echo hi\r\n\x1b]133;C\x07hi\r\n\x1b]133;D;0\x07");
        assert_eq!(t.take_completed_command(), Some("echo hi".to_string()));
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p zenith-core`
Expected: FAIL — `no method named take_completed_command` / `current_input`

- [ ] **Step 3: Implement the state machine**

In `crates/zenith-core/src/term.rs`, add above `struct TerminalState` (line 5):

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
enum ShellState {
    Ground,
    Prompt,
    Input,
    Running,
}
```

Add fields to `TerminalState` (after `last_char: Option<char>,` at line 22):

```rust
    shell_state: ShellState,
    input_start: Option<(usize, usize)>,
    pending_command: Option<String>,
    completed_commands: Vec<String>,
```

Initialize them in `Terminal::new` (after `last_char: None,` at line 50):

```rust
                shell_state: ShellState::Ground,
                input_start: None,
                pending_command: None,
                completed_commands: Vec::new(),
```

Add public methods to `impl Terminal` (after `reset_display_offset` at line 101-103):

```rust
    pub fn current_input(&self) -> Option<String> {
        if self.state.shell_state != ShellState::Input {
            return None;
        }
        let (col, abs_row) = self.state.input_start?;
        for c in self.state.cursor_col..self.state.grid.cols() {
            let cell = self.state.grid.cell(c, self.state.cursor_row);
            if cell.width != 0 && cell.c != ' ' && cell.c != '\0' {
                return None;
            }
        }
        let cur_abs = self.state.grid.scrollback_len() + self.state.cursor_row;
        self.state
            .extract_text(col, abs_row, self.state.cursor_col, cur_abs)
    }

    pub fn take_completed_command(&mut self) -> Option<String> {
        if self.state.completed_commands.is_empty() {
            None
        } else {
            Some(self.state.completed_commands.remove(0))
        }
    }
```

Add to `impl TerminalState` (after `parse_color_from_params`, before the closing brace at line 222):

```rust
    fn handle_shell_marker(&mut self, param: &[u8]) {
        match param.first() {
            Some(b'A') => {
                self.shell_state = ShellState::Prompt;
                self.input_start = None;
            }
            Some(b'B') => {
                self.shell_state = ShellState::Input;
                self.input_start =
                    Some((self.cursor_col, self.grid.scrollback_len() + self.cursor_row));
            }
            Some(b'C') => {
                if let Some((col, abs_row)) = self.input_start {
                    let end_abs = self.grid.scrollback_len() + self.cursor_row;
                    self.pending_command =
                        self.extract_text(col, abs_row, self.cursor_col, end_abs);
                }
                self.shell_state = ShellState::Running;
                self.input_start = None;
            }
            Some(b'D') => {
                if let Some(cmd) = self.pending_command.take() {
                    let cmd = cmd.trim().to_string();
                    if !cmd.is_empty() {
                        self.completed_commands.push(cmd);
                    }
                }
                self.shell_state = ShellState::Ground;
                self.input_start = None;
            }
            _ => {}
        }
    }

    // Positions are (col, abs_row) where abs_row = scrollback_len + screen_row.
    // end_col is exclusive. Returns None if the start row scrolled out of the grid.
    fn extract_text(
        &self,
        start_col: usize,
        start_abs_row: usize,
        end_col: usize,
        end_abs_row: usize,
    ) -> Option<String> {
        let sb = self.grid.scrollback_len();
        if start_abs_row < sb || end_abs_row < start_abs_row {
            return None;
        }
        let mut text = String::new();
        for abs_row in start_abs_row..=end_abs_row {
            let row = abs_row - sb;
            if row >= self.grid.rows() {
                break;
            }
            let from = if abs_row == start_abs_row { start_col } else { 0 };
            let to = if abs_row == end_abs_row {
                end_col.min(self.grid.cols())
            } else {
                self.grid.cols()
            };
            let mut line = String::new();
            for col in from..to {
                let cell = self.grid.cell(col, row);
                if cell.width != 0 {
                    line.push(cell.c);
                }
            }
            if abs_row == end_abs_row {
                text.push_str(&line);
            } else {
                text.push_str(line.trim_end());
            }
        }
        Some(text)
    }
```

Wire into `osc_dispatch` (line 254): add a match arm before the `_ => {}` arm:

```rust
            b"133" => {
                if params.len() > 1 {
                    self.handle_shell_marker(params[1]);
                }
            }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p zenith-core`
Expected: all pass (10 pre-existing + 7 new)

- [ ] **Step 5: Commit**

```bash
git add crates/zenith-core/src/term.rs
git commit -m "feat: OSC 133 shell state machine with command capture"
```

---

### Task 2: History store

**Files:**
- Create: `crates/zenith-core/src/history.rs`
- Modify: `crates/zenith-core/src/lib.rs`

Plain-text file, one command per line, 0600 permissions. Dedup: re-appending an existing command moves it to the end (most recent). `suggest(prefix)` scans newest-first for a `starts_with` match that is not the prefix itself. Whole file is rewritten on append (required by dedup reorder); best-effort I/O — history failure must never break the terminal.

- [ ] **Step 1: Write the failing tests**

Create `crates/zenith-core/src/history.rs`:

```rust
use std::fs;
use std::path::PathBuf;

const MAX_ENTRIES: usize = 10_000;

pub struct History {
    entries: Vec<String>,
    path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("zenith_history_{}_{}", std::process::id(), name));
        let _ = fs::remove_file(&p);
        p
    }

    #[test]
    fn load_missing_file_gives_empty() {
        let h = History::load(temp_path("missing"));
        assert_eq!(h.suggest("ls"), None);
    }

    #[test]
    fn suggest_most_recent_first() {
        let mut h = History::load(temp_path("recent"));
        h.append("git status");
        h.append("git stash");
        assert_eq!(h.suggest("git st"), Some("git stash"));
        h.append("git status"); // moves to most-recent
        assert_eq!(h.suggest("git st"), Some("git status"));
    }

    #[test]
    fn suggest_skips_exact_match_and_empty_prefix() {
        let mut h = History::load(temp_path("exact"));
        h.append("ls");
        assert_eq!(h.suggest("ls"), None);
        assert_eq!(h.suggest(""), None);
        assert_eq!(h.suggest("   "), None);
        assert_eq!(h.suggest("l"), Some("ls"));
    }

    #[test]
    fn append_dedups_and_ignores_empty() {
        let mut h = History::load(temp_path("dedup"));
        h.append("make build");
        h.append("  ");
        h.append("make build");
        assert_eq!(h.entries.len(), 1);
    }

    #[test]
    fn persists_across_reload_with_0600() {
        use std::os::unix::fs::PermissionsExt;
        let path = temp_path("persist");
        let mut h = History::load(path.clone());
        h.append("cargo test");
        drop(h);
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let h2 = History::load(path);
        assert_eq!(h2.suggest("cargo"), Some("cargo test"));
    }
}
```

Register the module in `crates/zenith-core/src/lib.rs`:

```rust
pub mod cell;
pub mod grid;
pub mod history;
pub mod term;
pub mod pty;
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p zenith-core history`
Expected: FAIL — `no function or associated item named load` (compile error)

- [ ] **Step 3: Implement History**

Add to `crates/zenith-core/src/history.rs` (between the struct and the tests module):

```rust
impl History {
    pub fn load(path: PathBuf) -> Self {
        let entries = fs::read_to_string(&path)
            .map(|s| {
                s.lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default();
        Self { entries, path }
    }

    pub fn default_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
        PathBuf::from(home).join(".config/zenith/history")
    }

    pub fn append(&mut self, command: &str) {
        let command = command.trim();
        if command.is_empty() {
            return;
        }
        self.entries.retain(|e| e != command);
        self.entries.push(command.to_string());
        if self.entries.len() > MAX_ENTRIES {
            let excess = self.entries.len() - MAX_ENTRIES;
            self.entries.drain(..excess);
        }
        self.persist();
    }

    pub fn suggest(&self, prefix: &str) -> Option<&str> {
        if prefix.trim().is_empty() {
            return None;
        }
        self.entries
            .iter()
            .rev()
            .find(|e| e.starts_with(prefix) && e.as_str() != prefix)
            .map(|s| s.as_str())
    }

    fn persist(&self) {
        if let Some(dir) = self.path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let data = self.entries.join("\n") + "\n";
        let _ = fs::write(&self.path, data);
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&self.path, fs::Permissions::from_mode(0o600));
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p zenith-core`
Expected: all pass

- [ ] **Step 5: Commit**

```bash
git add crates/zenith-core/src/history.rs crates/zenith-core/src/lib.rs
git commit -m "feat: persistent command history with prefix suggestions"
```

---

### Task 3: Shell integration script + installer + env var

**Files:**
- Create: `crates/zenith-core/resources/shell-integration.sh`
- Create: `crates/zenith-core/src/shell_integration.rs`
- Modify: `crates/zenith-core/src/lib.rs`
- Modify: `crates/zenith-core/src/pty.rs`

One script handles both shells ($ZSH_VERSION / $BASH_VERSION detection). zsh: `precmd` emits `D;<exit>` + `A`, `preexec` emits `C`, `B` is appended to PS1 inside `%{...%}`. bash: `PROMPT_COMMAND` emits `D;<exit>` + `A`, `PS0` emits `C`, `B` appended to PS1 inside `\[...\]`. `printf '\033...\007'` (octal, POSIX-portable — macOS ships bash 3.2). Guard variable is NOT exported so nested shells re-source it themselves.

- [ ] **Step 1: Write the script**

Create `crates/zenith-core/resources/shell-integration.sh`:

```sh
# Zenith shell integration — emits OSC 133 prompt markers.
# Installed by Zenith to ~/.config/zenith/shell-integration.sh (overwritten on update).
# Enable by adding to your ~/.zshrc or ~/.bashrc:
#   [ -n "$ZENITH_SHELL_INTEGRATION" ] && . ~/.config/zenith/shell-integration.sh

if [ -n "$ZENITH_INTEGRATION_LOADED" ]; then
    return 0
fi
ZENITH_INTEGRATION_LOADED=1

if [ -n "$ZSH_VERSION" ]; then
    _zenith_precmd() {
        local ret=$?
        printf '\033]133;D;%s\007' "$ret"
        printf '\033]133;A\007'
    }
    _zenith_preexec() {
        printf '\033]133;C\007'
    }
    typeset -ag precmd_functions preexec_functions
    precmd_functions+=(_zenith_precmd)
    preexec_functions+=(_zenith_preexec)
    PS1="$PS1"$'%{\033]133;B\007%}'
elif [ -n "$BASH_VERSION" ]; then
    _zenith_prompt_command() {
        local ret=$?
        printf '\033]133;D;%s\007' "$ret"
        printf '\033]133;A\007'
    }
    PROMPT_COMMAND="_zenith_prompt_command${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
    PS1="$PS1\[\033]133;B\007\]"
    PS0='\033]133;C\007'"$PS0"
fi
```

- [ ] **Step 2: Syntax-check the script under both shells**

Run: `bash -n crates/zenith-core/resources/shell-integration.sh && zsh -n crates/zenith-core/resources/shell-integration.sh && echo OK`
Expected: `OK`

- [ ] **Step 3: Write the failing installer tests**

Create `crates/zenith-core/src/shell_integration.rs`:

```rust
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const INTEGRATION_SCRIPT: &str = include_str!("../resources/shell-integration.sh");

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("zenith_shellint_{}_{}", std::process::id(), name));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn install_writes_script() {
        let dir = temp_dir("write");
        let path = install_to(&dir).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), INTEGRATION_SCRIPT);
    }

    #[test]
    fn install_restores_modified_script() {
        let dir = temp_dir("restore");
        let path = install_to(&dir).unwrap();
        fs::write(&path, "tampered").unwrap();
        let path2 = install_to(&dir).unwrap();
        assert_eq!(path, path2);
        assert_eq!(fs::read_to_string(&path).unwrap(), INTEGRATION_SCRIPT);
    }
}
```

- [ ] **Step 4: Run tests to verify they fail**

Run: `cargo test -p zenith-core shell_integration`
Expected: FAIL — `cannot find function install_to` (compile error). Note: `lib.rs` must first gain `pub mod shell_integration;` (add it now, after `pub mod pty;`).

- [ ] **Step 5: Implement the installer**

Add to `crates/zenith-core/src/shell_integration.rs` (between the const and tests):

```rust
pub fn install_to(dir: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let path = dir.join("shell-integration.sh");
    let up_to_date = fs::read_to_string(&path)
        .map(|cur| cur == INTEGRATION_SCRIPT)
        .unwrap_or(false);
    if !up_to_date {
        fs::write(&path, INTEGRATION_SCRIPT)?;
    }
    Ok(path)
}

pub fn ensure_installed() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    install_to(&PathBuf::from(home).join(".config/zenith")).ok()
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo test -p zenith-core`
Expected: all pass

- [ ] **Step 7: Wire into PTY spawn**

In `crates/zenith-core/src/pty.rs`, at the top of `Pty::spawn` (after line 21 `pub fn spawn(...) -> io::Result<Self> {`), add:

```rust
        let _ = crate::shell_integration::ensure_installed();
```

Then extend the env chain (lines 64-70) — filter the flag from inherited env and add it:

```rust
        let envs: Vec<CString> = std::env::vars()
            .filter(|(k, _)| k != "TERM" && k != "ZENITH_SHELL_INTEGRATION")
            .filter_map(|(k, v)| CString::new(format!("{}={}", k, v)).ok())
            .chain(std::iter::once(
                CString::new("TERM=xterm-256color").unwrap(),
            ))
            .chain(std::iter::once(
                CString::new("ZENITH_SHELL_INTEGRATION=1").unwrap(),
            ))
            .collect();
```

- [ ] **Step 8: Build and test workspace**

Run: `cargo test -p zenith-core && cargo build`
Expected: all pass, clean build

- [ ] **Step 9: Commit**

```bash
git add crates/zenith-core/resources/shell-integration.sh crates/zenith-core/src/shell_integration.rs crates/zenith-core/src/lib.rs crates/zenith-core/src/pty.rs
git commit -m "feat: bash/zsh OSC 133 integration script with idempotent installer"
```

---

### Task 4: Ghost text rendering

**Files:**
- Modify: `crates/zenith-render/src/vertex.rs`
- Modify: `crates/zenith-render/Cargo.toml`
- Modify: `crates/zenith-ffi/src/lib.rs` (call site only — pass `None` for now)

`generate_render_data` gains a `suggestion: Option<&str>` parameter (last position). Ghost glyphs are emitted after the main grid pass, starting at the cursor cell, regular weight, color `GHOST_TEXT_COLOR` (DEFAULT_FG 0.784 scaled to ~40% brightness). Only when `display_offset == 0` (viewing the live screen); stops at end of row. Pure display — the grid is never touched. The zenith-ffi call site is updated in the same commit (passing `None`) so the workspace keeps compiling; real wiring lands in Task 5.

- [ ] **Step 1: Add unicode-width dependency**

In `crates/zenith-render/Cargo.toml`, add to `[dependencies]`:

```toml
unicode-width = "0.2"
```

- [ ] **Step 2: Write the failing tests**

Append to `crates/zenith-render/src/vertex.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (Grid, FontContext, GlyphAtlas) {
        (
            Grid::new(20, 5, 100),
            FontContext::new("Menlo", 12.0),
            GlyphAtlas::new(512, 512),
        )
    }

    #[test]
    fn ghost_text_renders_dim_at_cursor() {
        let (grid, mut font_ctx, mut atlas) = setup();
        let out = generate_render_data(
            &grid, &mut font_ctx, &mut atlas, (3, 0), true, 0.0, 0.0, Some("ls"),
        );
        // empty grid → the only glyphs are the 2 ghost chars
        assert_eq!(out.glyph_instances.len(), 2);
        assert!(out
            .glyph_instances
            .iter()
            .all(|g| g.color == GHOST_TEXT_COLOR));
        // first ghost glyph sits in the cursor column
        let cell_w = font_ctx.cell_width;
        assert!(out.glyph_instances[0].position[0] >= 3.0 * cell_w);
        assert!(out.glyph_instances[0].position[0] < 4.0 * cell_w);
    }

    #[test]
    fn ghost_text_clipped_at_row_end() {
        let (grid, mut font_ctx, mut atlas) = setup();
        let out = generate_render_data(
            &grid, &mut font_ctx, &mut atlas, (18, 0), true, 0.0, 0.0, Some("abcdef"),
        );
        assert_eq!(out.glyph_instances.len(), 2); // cols 18,19 only
    }

    #[test]
    fn ghost_text_hidden_while_scrolled() {
        let (mut grid, mut font_ctx, mut atlas) = setup();
        // create scrollback then scroll up
        for _ in 0..3 {
            grid.scroll_up(0, 4, 1);
        }
        grid.scroll_display(2);
        let out = generate_render_data(
            &grid, &mut font_ctx, &mut atlas, (0, 0), true, 0.0, 0.0, Some("ls"),
        );
        assert_eq!(out.glyph_instances.len(), 0);
    }

    #[test]
    fn no_suggestion_no_ghost() {
        let (grid, mut font_ctx, mut atlas) = setup();
        let out =
            generate_render_data(&grid, &mut font_ctx, &mut atlas, (0, 0), true, 0.0, 0.0, None);
        assert_eq!(out.glyph_instances.len(), 0);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p zenith-render`
Expected: FAIL — function takes 7 arguments but 8 were supplied / `GHOST_TEXT_COLOR` not found

- [ ] **Step 4: Implement ghost text emission**

In `crates/zenith-render/src/vertex.rs`, add after the `RenderOutput` struct (line 36):

```rust
pub const GHOST_TEXT_COLOR: [f32; 4] = [0.31, 0.31, 0.33, 1.0];
```

Change the `generate_render_data` signature (line 38-46) to add the trailing parameter:

```rust
pub fn generate_render_data(
    grid: &Grid,
    font_ctx: &mut FontContext,
    atlas: &mut GlyphAtlas,
    cursor: (usize, usize),
    show_cursor: bool,
    _viewport_width: f32,
    _viewport_height: f32,
    suggestion: Option<&str>,
) -> RenderOutput {
```

Insert the ghost pass after the main grid loop (after line 100's closing `}`, before the `cursor_inst` block):

```rust
    if let Some(sug) = suggestion {
        if grid.display_offset() == 0 {
            let mut col = cursor.0;
            let row = cursor.1;
            let y = row as f32 * cell_h;
            for c in sug.chars() {
                let width = unicode_width::UnicodeWidthChar::width(c).unwrap_or(1);
                if width == 0 || col + width > grid.cols() {
                    if width == 0 {
                        continue;
                    }
                    break;
                }
                if c != ' ' {
                    let key = GlyphKey {
                        c,
                        bold: false,
                        italic: false,
                    };
                    if let Some(entry) = atlas.get_or_insert(key, font_ctx) {
                        let x = col as f32 * cell_w;
                        glyph_instances.push(GlyphInstance {
                            position: [
                                x + entry.bearing_x,
                                y + (font_ctx.baseline - entry.bearing_y),
                            ],
                            size: [entry.width as f32, entry.height as f32],
                            tex_offset: [entry.x as f32 / atlas_w, entry.y as f32 / atlas_h],
                            tex_size: [
                                entry.width as f32 / atlas_w,
                                entry.height as f32 / atlas_h,
                            ],
                            color: GHOST_TEXT_COLOR,
                        });
                    }
                }
                col += width;
            }
        }
    }
```

- [ ] **Step 5: Fix the zenith-ffi call site (keep workspace green)**

In `crates/zenith-ffi/src/lib.rs`, `zn_terminal_render` (line 161-169), add the trailing argument:

```rust
    let output = generate_render_data(
        term.term.grid(),
        &mut term.font_ctx,
        &mut term.atlas,
        cursor,
        show_cursor,
        viewport_width,
        viewport_height,
        None,
    );
```

- [ ] **Step 6: Run tests and build workspace**

Run: `cargo test -p zenith-render && cargo build`
Expected: 4 new tests pass, workspace builds

- [ ] **Step 7: Commit**

```bash
git add crates/zenith-render/src/vertex.rs crates/zenith-render/Cargo.toml crates/zenith-ffi/src/lib.rs
git commit -m "feat: dim ghost-text rendering for autosuggestions"
```

---

### Task 5: FFI glue — history wiring + accept API

**Files:**
- Modify: `crates/zenith-ffi/src/lib.rs`
- Modify: `Zenith/Sources/CZenith/zenith.h`

`ZenithTerminal` gains a `History`. `zn_terminal_read` drains completed commands into history after feeding. `zn_terminal_render` computes the ghost remainder (`current_input` → `history.suggest` → strip prefix; byte slicing is safe because `starts_with` guarantees a char boundary). New `zn_terminal_accept_suggestion` returns the remainder as a C string (caller frees with `zn_string_free`) or NULL; all gating (Input state, nothing right of cursor) already lives in `current_input()`.

- [ ] **Step 1: Add History to ZenithTerminal**

In `crates/zenith-ffi/src/lib.rs`:

Add import (after line 3 `use zenith_core::pty::Pty;`):

```rust
use zenith_core::history::History;
```

Extend the struct (line 35-40):

```rust
pub struct ZenithTerminal {
    term: Terminal,
    pty: Pty,
    font_ctx: FontContext,
    atlas: GlyphAtlas,
    history: History,
}
```

In `zn_terminal_new` (line 74-79), add the field:

```rust
    let terminal = Box::new(ZenithTerminal {
        term: Terminal::new(cols as usize, rows as usize),
        pty,
        font_ctx,
        atlas,
        history: History::load(History::default_path()),
    });
```

- [ ] **Step 2: Drain completed commands in zn_terminal_read**

Replace the body of the `Ok(n)` arm (line 99-102):

```rust
        Ok(n) => {
            term.term.feed(&buf[..n]);
            while let Some(cmd) = term.term.take_completed_command() {
                term.history.append(&cmd);
            }
            true
        }
```

- [ ] **Step 3: Compute the ghost remainder in zn_terminal_render**

In `zn_terminal_render`, before the `generate_render_data` call, add:

```rust
    let ghost: Option<String> = term.term.current_input().and_then(|input| {
        term.history
            .suggest(&input)
            .map(|full| full[input.len()..].to_string())
    });
```

and change the call's last argument from `None` to:

```rust
        ghost.as_deref(),
```

- [ ] **Step 4: Add the accept API**

Add after `zn_terminal_screen_text` (line 291):

```rust
#[no_mangle]
pub extern "C" fn zn_terminal_accept_suggestion(term: *mut ZenithTerminal) -> *mut c_char {
    if term.is_null() {
        return std::ptr::null_mut();
    }
    let term = unsafe { &mut *term };
    let input = match term.term.current_input() {
        Some(i) => i,
        None => return std::ptr::null_mut(),
    };
    let remainder = match term.history.suggest(&input) {
        Some(full) => full[input.len()..].to_string(),
        None => return std::ptr::null_mut(),
    };
    if remainder.is_empty() {
        return std::ptr::null_mut();
    }
    CString::new(remainder)
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}
```

- [ ] **Step 5: Declare in the header**

In `Zenith/Sources/CZenith/zenith.h`, after the `zn_terminal_screen_text` declaration (line 76), add:

```c
char *zn_terminal_accept_suggestion(ZenithTerminal *term);
```

- [ ] **Step 6: Build and run full checks**

Run: `cargo build && cargo test --workspace && cargo clippy --workspace -- -D warnings`
Expected: build OK, all tests pass, no clippy warnings

- [ ] **Step 7: Commit**

```bash
git add crates/zenith-ffi/src/lib.rs Zenith/Sources/CZenith/zenith.h
git commit -m "feat: wire history suggestions through FFI with accept API"
```

---

### Task 6: Swift Right-arrow accept + end-to-end verification

**Files:**
- Modify: `Zenith/Sources/Zenith/TerminalView.swift`

Right-arrow (keyCode 124) with no modifiers first tries to accept a suggestion; if the FFI returns a remainder it is written to the PTY (the shell echoes it back into the grid — no direct grid writes) and the event is consumed. Otherwise falls through to the existing `specialKeySequence` path which sends CSI C. The remainder can never contain `\r`/`\n`, so acceptance can never execute anything.

- [ ] **Step 1: Add the accept hook in keyDown**

In `Zenith/Sources/Zenith/TerminalView.swift`, inside `keyDown` — after `let modifiers = event.modifierFlags` (line 222) and before the `if let special = specialKeySequence(event)` block (line 224) — insert:

```swift
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
```

- [ ] **Step 2: Build everything**

Run: `make build`
Expected: `Rust build complete` + `Swift build complete`

- [ ] **Step 3: One-time shell setup for verification**

The tester adds this line to `~/.zshrc` (documented in the script header; v1 is manual by spec):

```sh
[ -n "$ZENITH_SHELL_INTEGRATION" ] && . ~/.config/zenith/shell-integration.sh
```

- [ ] **Step 4: Manual verification checklist (launch with `make run`)**

1. `ls -la ~/.config/zenith/` → `shell-integration.sh` exists; after running any command, `history` exists with `-rw-------` (0600).
2. Run `echo hello-zenith` once. Press Enter to get a fresh prompt. Type `echo` → dim ghost ` hello-zenith` appears after the cursor.
3. Press → → the full command materializes as real (bright) text; the shell cursor is at end of line; NOTHING executed. Press Enter → `hello-zenith` prints.
4. Type `echo`, then press ← (cursor mid-input) → ghost disappears; → moves the cursor normally.
5. Type a prefix with no history match (e.g. `zzz`) → no ghost; → does nothing (cursor at EOL).
6. Scroll up with the trackpad while a ghost is visible → ghost disappears; scroll back to bottom → reappears.
7. Run a command that fails (`false`), then a new command — history still records correctly (D;1 handled).
8. Regression: arrow keys in `vim` and shell history navigation (↑/↓) still work; Cmd+K AI panel still opens; selection copy still works.

- [ ] **Step 5: Commit**

```bash
git add Zenith/Sources/Zenith/TerminalView.swift
git commit -m "feat: accept autosuggestion with right arrow"
```

---

## Self-Review Notes

- **Spec coverage:** OSC 133 A/B/C/D parsing (T1), history file 0600 + dedup + most-recent-first prefix match (T2), single distributed script for bash+zsh + ZENITH_SHELL_INTEGRATION env + manual source line (T3), dim ghost text display-layer-only (T4), Right-arrow accept at end of input (T5+T6), Rust unit tests for the OSC 133 state machine (T1). Non-goals untouched.
- **Type consistency:** `current_input() -> Option<String>`, `take_completed_command() -> Option<String>`, `suggest(&str) -> Option<&str>`, `install_to(&Path) -> io::Result<PathBuf>`, `generate_render_data(..., suggestion: Option<&str>)`, `zn_terminal_accept_suggestion(*mut ZenithTerminal) -> *mut c_char` — used identically across tasks.
- **Compile greenness:** T4 updates the zenith-ffi call site with `None` in the same commit as the signature change; every commit builds.
