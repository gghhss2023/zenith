# Contributing to Zenith

Thanks for your interest in contributing!

## Getting started

```bash
git clone https://github.com/gghhss2023/zenith.git
cd zenith
make build     # debug build (Rust + Swift)
make check     # cargo test + clippy — must pass before submitting
```

**Requirements:** macOS 13+, Rust toolchain, Xcode Command Line Tools.

## Project layout

| Path | What lives there |
|---|---|
| `crates/zenith-core` | Terminal state machine: VTE parsing, grid, scrollback, PTY, shell integration |
| `crates/zenith-render` | Font rasterization, glyph atlas, GPU instance generation |
| `crates/zenith-ffi` | C ABI consumed by Swift (keep in sync with `Zenith/Sources/CZenith/zenith.h`) |
| `Zenith/` | SwiftPM app: AppKit, Metal pipeline, input handling |

## Pull requests

1. Fork and create a feature branch from `main`
2. Keep changes focused — one feature or fix per PR
3. Add tests for core (Rust) changes; `make check` must pass
4. For UI/rendering changes, include a screenshot or short recording
5. Write commit messages in English, imperative mood (`fix: ...`, `feat: ...`)

## Reporting bugs

Open an issue with:

- macOS version and Zenith version (`About Zenith`)
- Steps to reproduce — the exact commands you ran help a lot
- What you expected vs. what happened (screenshots welcome)

Terminal escape-sequence bugs are much easier to fix with a byte capture:

```bash
# record a session that reproduces the bug, then attach /tmp/rec to the issue
script -q /tmp/rec <your-command>
```

## License

By contributing, you agree that your contributions will be licensed under the [MIT License](LICENSE).
