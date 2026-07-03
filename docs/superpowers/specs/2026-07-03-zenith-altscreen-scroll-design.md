# Zenith Alt-Screen Scroll Wheel Design

## Goal

Make the scroll wheel useful inside full-screen programs (vim, less, htop). These
programs run on the alternate screen (DECSET 1049), which has no scrollback by
design, so display scrolling is a no-op there. Instead, translate wheel events
into Up/Down arrow key sequences sent to the program — the same behavior iTerm2
ships as "Scroll wheel sends arrow keys when in alternate screen mode".

## Decisions

- **Detection:** the terminal is on the alt screen iff `TerminalState.alt_grid.is_some()`
  (the main grid is stashed there on `ESC[?1049h` and restored on `ESC[?1049l`).
  Expose as `Terminal::is_alt_screen()`.
- **Translation point:** inside the FFI `zn_terminal_scroll_display`. On alt screen,
  write arrow sequences to the PTY instead of calling `scroll_display`. The Swift
  layer stays unchanged (its `needsDisplay = true` after the call is harmless; the
  PTY read path already triggers redraws).
- **Sequences:** `delta > 0` (wheel up) → `ESC [ A` repeated `delta` times;
  `delta < 0` (wheel down) → `ESC [ B` repeated `|delta|` times. Always CSI form:
  Zenith's `specialKeySequence` already sends CSI arrows regardless of DECCKM and
  vim/less accept them (verified by daily use).
- **Sign convention:** matches existing `Grid::scroll_display` — positive delta means
  scroll back/up.

## Non-goals

- Mouse reporting modes (DECSET 1000/1002/1006) — Zenith has no mouse protocol
  support; out of scope.
- DECCKM (application cursor keys, `ESC O A` variant) — not tracked today; CSI works.
- Scroll acceleration/paging (e.g., Page Up/Down for large deltas).

## Testing

- Rust unit tests in `term.rs`: `is_alt_screen()` is false initially, true after
  feeding `\x1b[?1049h`, false again after `\x1b[?1049l`.
- The FFI branch is thin glue over `Pty::write_all`; verified manually in the GUI:
  open vim with a long file, wheel scrolls the buffer; `less` likewise; normal
  shell scrollback behavior unchanged.
