# Alt-Screen Scroll Wheel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Scroll wheel works inside vim/less/htop by translating wheel events into Up/Down arrow key sequences while the terminal is on the alternate screen.

**Architecture:** `Terminal` gains an `is_alt_screen()` getter (alt screen ⇔ `alt_grid.is_some()`). The FFI `zn_terminal_scroll_display` branches on it: on alt screen it writes `ESC[A`/`ESC[B` repeated `|delta|` times to the PTY; otherwise it scrolls the display as today. Swift layer unchanged.

**Tech Stack:** Rust (zenith-core, zenith-ffi). No Swift changes.

---

### Task 1: `Terminal::is_alt_screen()` getter

**Files:**
- Modify: `crates/zenith-core/src/term.rs`

**Context:** `TerminalState` (fields around line 20-35) has `alt_grid: Option<Grid>`. `enter_alt_screen` (line ~207) stashes the main grid into `alt_grid`; `exit_alt_screen` (line ~216) restores it. So `alt_grid.is_some()` ⇔ currently on the alt screen. Public getters on `Terminal` delegate to `self.state` (see `pub fn grid(&self)` at line ~78). Tests live in a `#[cfg(test)] mod tests` at the bottom of the file; existing tests construct `Terminal::new(cols, rows)` and drive it with `t.feed(b"...")` (see `current_input_cleared_by_alt_screen` at line ~758 which feeds `\x1b[?1049h`).

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` in `crates/zenith-core/src/term.rs`:

```rust
#[test]
fn is_alt_screen_tracks_1049() {
    let mut t = Terminal::new(10, 4);
    assert!(!t.is_alt_screen());
    t.feed(b"\x1b[?1049h");
    assert!(t.is_alt_screen());
    t.feed(b"\x1b[?1049l");
    assert!(!t.is_alt_screen());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p zenith-core is_alt_screen_tracks_1049`
Expected: FAIL to compile with "no method named `is_alt_screen`"

- [ ] **Step 3: Write minimal implementation**

In `impl Terminal`, next to the other getters (after `pub fn show_cursor` around line 86-88), add:

```rust
pub fn is_alt_screen(&self) -> bool {
    self.state.alt_grid.is_some()
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p zenith-core is_alt_screen_tracks_1049`
Expected: PASS

- [ ] **Step 5: Run the full core test suite**

Run: `cargo test -p zenith-core`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/zenith-core/src/term.rs
git commit -m "feat: expose alt-screen state on Terminal"
```

---

### Task 2: FFI scroll-to-arrow translation

**Files:**
- Modify: `crates/zenith-ffi/src/lib.rs:125-128` (`zn_terminal_scroll_display`)

**Context:** Current implementation:

```rust
#[no_mangle]
pub extern "C" fn zn_terminal_scroll_display(term: *mut ZenithTerminal, delta: i32) {
    let term = unsafe { &mut *term };
    term.term.scroll_display(delta);
}
```

`ZenithTerminal` owns both `term: Terminal` and `pty: Pty`. `Pty::write_all` is already used by `zn_terminal_write` (line ~117). Sign convention: positive delta = wheel up. On the alt screen, wheel up must send Up arrows (`ESC [ A` = `\x1b[A`), wheel down must send Down arrows (`\x1b[B`).

- [ ] **Step 1: Replace the function body**

```rust
#[no_mangle]
pub extern "C" fn zn_terminal_scroll_display(term: *mut ZenithTerminal, delta: i32) {
    let term = unsafe { &mut *term };
    if term.term.is_alt_screen() {
        if delta == 0 {
            return;
        }
        let seq: &[u8] = if delta > 0 { b"\x1b[A" } else { b"\x1b[B" };
        let n = delta.unsigned_abs() as usize;
        let mut buf = Vec::with_capacity(seq.len() * n);
        for _ in 0..n {
            buf.extend_from_slice(seq);
        }
        let _ = term.pty.write_all(&buf);
    } else {
        term.term.scroll_display(delta);
    }
}
```

- [ ] **Step 2: Verify the workspace builds and tests pass**

Run: `cargo build && cargo test`
Expected: builds cleanly; all tests pass (no new unit test — this is thin glue over `Pty::write_all`, covered by manual GUI verification below)

- [ ] **Step 3: Commit**

```bash
git add crates/zenith-ffi/src/lib.rs
git commit -m "feat: translate scroll wheel to arrow keys on alt screen"
```

---

### Manual GUI verification (controller + user, after both tasks)

1. Rebuild the app (`swift build -c release` in `Zenith/` per project build flow) and relaunch.
2. In Zenith: `vim` on a file longer than one screen → wheel up/down moves the buffer.
3. `less` on a long file → wheel scrolls.
4. Quit vim → normal shell scrollback via wheel still works.
