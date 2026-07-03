# Zenith App Bundle Design

## Goal

Ship Zenith as a real macOS application: `Zenith.app` with an icon, installable
into /Applications, launchable from Finder/Dock/Spotlight. Today the app is a bare
SwiftPM executable that dynamically links the *debug* Rust dylib out of the source
tree — it cannot run outside the repo.

## Decisions

- **Static linking.** `Package.swift` links `../target/release/libzenith_ffi.a` by
  explicit file path (avoids ld's dylib preference) and drops the `-rpath` flag.
  Both debug and release Swift builds consume the release Rust staticlib; the
  resulting binary is self-contained and relocatable. `make build` runs
  `cargo build --release` accordingly. Any undefined-symbol link errors are fixed
  by adding the missing system frameworks to `linkerSettings`.
- **Launch working directory.** At startup the app calls
  `FileManager.default.changeCurrentDirectoryPath(NSHomeDirectory())` so shells
  spawned from a Finder launch start in `$HOME` instead of `/` (matches
  Terminal.app). The PTY already spawns a login shell (`-bash`), so PATH and
  profiles work from Finder launches unchanged.
- **Icon.** `scripts/make_icon.swift` (run manually, output committed) draws a
  1024×1024 macOS-style rounded-rect icon: dark background matching the terminal
  theme, a pink `❯` prompt (cursor color #F87890-ish) and a light cursor block.
  `sips` scales the standard sizes, `iconutil` produces
  `Zenith/Resources/Zenith.icns`, which is committed to the repo.
- **Bundle assembly.** `make app` builds release and assembles
  `dist/Zenith.app/Contents/{MacOS/Zenith, Info.plist, Resources/Zenith.icns}`,
  then ad-hoc codesigns (`codesign --force --deep -s -`). Info.plist keys:
  identifier `io.github.gghhss2023.zenith`, name/executable/icon `Zenith`,
  `LSMinimumSystemVersion 14.0`, `NSHighResolutionCapable`, version 0.1.0.
- **Install & DMG.** `make install` ditto-copies the bundle to
  `/Applications/Zenith.app` (replacing any old copy). `make dmg` builds
  `dist/Zenith-0.1.0.dmg` via `hdiutil` from a staging folder containing the app
  and an /Applications symlink.

## Non-goals

- Developer ID signing / notarization (requires a paid Apple Developer account;
  ad-hoc signature is fine for a locally built personal app).
- Universal binary (build machine is x86_64; single-arch only).
- Auto-update (Sparkle), preference UI, multi-window session restore.

## Testing

- `cargo test` + `swift build` stay green; app binary runs from inside the bundle.
- `codesign --verify` passes on the bundle.
- Launch `/Applications/Zenith.app` via `open`: process runs from /Applications,
  window appears, shell starts in `$HOME`, typing/scroll/AI panel still work.
- User confirms Dock icon looks right and double-click launch works.
