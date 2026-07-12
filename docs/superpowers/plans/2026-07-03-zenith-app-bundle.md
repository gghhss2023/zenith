# Zenith App Bundle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship Zenith as a self-contained `Zenith.app` with an icon, installable to /Applications, launchable from Finder/Dock, plus a DMG.

**Architecture:** Statically link the release Rust staticlib into the Swift binary (no rpath, relocatable). Chdir to `$HOME` at startup so Finder launches start shells in the home directory. A committed `.icns` (generated once by a script) plus an `Info.plist` are assembled with the release binary into `dist/Zenith.app` by `make app`, ad-hoc codesigned; `make install` copies to /Applications; `make dmg` wraps it via `hdiutil`.

**Tech Stack:** SwiftPM linkerSettings, AppKit (icon drawing), sips/iconutil, codesign, ditto, hdiutil, Make.

---

### Task 1: Static linking of the release Rust staticlib

**Files:**
- Modify: `Zenith/Package.swift:17-26` (linkerSettings)
- Modify: `Makefile:3-11` (build/release targets)

**Context:** Today `Package.swift` links with `-L ../target/debug -lzenith_ffi` plus an rpath into the source tree. Because `target/debug` contains BOTH `libzenith_ffi.dylib` and `libzenith_ffi.a` (crate-type is `["cdylib", "staticlib"]`), ld prefers the dylib, so the binary only runs next to the source tree. Fix: pass the release `.a` as an explicit input file (bypasses `-l` dylib preference), drop the rpath. Both debug and release Swift builds consume the release Rust staticlib. `swift build` always runs with CWD = `Zenith/`, so the relative path `../target/release/...` is stable. Statically linking pulls in the Rust crates' native deps (core-text, core-graphics used by zenith-render), so add CoreText/CoreGraphics/CoreFoundation frameworks up front.

- [ ] **Step 1: Replace linkerSettings in `Zenith/Package.swift`**

Replace the existing `linkerSettings` array (lines 17-26) with:

```swift
            linkerSettings: [
                .unsafeFlags([
                    "../target/release/libzenith_ffi.a",
                ]),
                .linkedFramework("Metal"),
                .linkedFramework("MetalKit"),
                .linkedFramework("AppKit"),
                .linkedFramework("CoreText"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("CoreFoundation"),
            ]
```

- [ ] **Step 2: Update Makefile build targets to always build release Rust**

Replace the `build:` and `release:` targets (Makefile lines 3-11) with:

```make
build:
	cargo build --release
	@echo "Rust build complete"
	cd Zenith && swift build
	@echo "Swift build complete"

release:
	cargo build --release
	cd Zenith && swift build -c release
```

- [ ] **Step 3: Build and fix any undefined symbols**

Run: `cargo build --release && cd Zenith && swift build -c release`
Expected: builds cleanly. If ld reports undefined symbols (e.g. `_SecXXX`, `_CFXXX`), add the corresponding `.linkedFramework("...")` (Security, CoreServices, etc.) to linkerSettings and rebuild until clean.

- [ ] **Step 4: Verify the binary is relocatable**

Run: `otool -L Zenith/.build/release/Zenith | grep -c zenith_ffi`
Expected: `0` (no dynamic reference to libzenith_ffi).

Run: `cp Zenith/.build/release/Zenith /tmp/zenith-reloc-test && /tmp/zenith-reloc-test & sleep 2 && kill %1 || true`
Expected: process starts without dyld errors (a window may flash; that's fine). Then `rm /tmp/zenith-reloc-test`.

- [ ] **Step 5: Run tests**

Run: `cargo test --workspace`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add Zenith/Package.swift Makefile
git commit -m "build: statically link release Rust staticlib into Swift binary"
```

---

### Task 2: Start shells in $HOME

**Files:**
- Modify: `Zenith/Sources/Zenith/ZenithApp.swift:7-12` (`main()`)

**Context:** The PTY child inherits the app's CWD (pty.rs never chdirs). Launched from Finder, CWD is `/`, so shells start in `/`. Terminal.app starts shells in `$HOME`; match that. The PTY already spawns a login shell (`-bash`), so PATH/profiles are fine.

- [ ] **Step 1: Add chdir at the top of `main()`**

In `ZenithApp.main()` (line 7), add as the first line of the function body:

```swift
        FileManager.default.changeCurrentDirectoryPath(NSHomeDirectory())
```

Result:

```swift
    static func main() {
        FileManager.default.changeCurrentDirectoryPath(NSHomeDirectory())
        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
    }
```

(`FileManager`/`NSHomeDirectory` come via the existing `import AppKit`.)

- [ ] **Step 2: Build**

Run: `cd Zenith && swift build -c release`
Expected: builds cleanly.

- [ ] **Step 3: Commit**

```bash
git add Zenith/Sources/Zenith/ZenithApp.swift
git commit -m "feat: start shells in home directory"
```

---

### Task 3: App icon (generated once, .icns committed)

**Files:**
- Create: `scripts/make_icon.swift`
- Create: `Zenith/Resources/Zenith.icns` (generated, committed)
- Modify: `.gitignore` (add `dist/`)

**Context:** Icon = dark rounded rect matching the terminal background (`#1A1B26`-ish, rgb 0.102/0.106/0.149), a pink `❯` prompt (cursor pink rgb 0.97/0.47/0.56) and a light cursor block. The script renders 1024×1024 PNG into `dist/`; `sips` + `iconutil` produce the `.icns`, which is committed so builds never need to regenerate it. `dist/` is scratch/output — gitignore it.

- [ ] **Step 1: Write `scripts/make_icon.swift`**

```swift
#!/usr/bin/env swift
import AppKit

let size: CGFloat = 1024
let image = NSImage(size: NSSize(width: size, height: size))
image.lockFocus()

let inset: CGFloat = size * 0.1
let rect = NSRect(x: inset, y: inset, width: size - 2 * inset, height: size - 2 * inset)
let path = NSBezierPath(roundedRect: rect, xRadius: rect.width * 0.225, yRadius: rect.width * 0.225)
NSColor(red: 0.102, green: 0.106, blue: 0.149, alpha: 1.0).setFill()
path.fill()

let promptFont = NSFont(name: "Menlo-Bold", size: 380) ?? NSFont.boldSystemFont(ofSize: 380)
let prompt = NSAttributedString(string: "❯", attributes: [
    .font: promptFont,
    .foregroundColor: NSColor(red: 0.97, green: 0.47, blue: 0.56, alpha: 1.0),
])
let promptSize = prompt.size()
prompt.draw(at: NSPoint(x: size * 0.26, y: (size - promptSize.height) / 2))

let cursorRect = NSRect(x: size * 0.54, y: size * 0.5 - 140, width: 160, height: 280)
NSColor(red: 0.75, green: 0.79, blue: 0.96, alpha: 1.0).setFill()
cursorRect.fill()

image.unlockFocus()

guard let tiff = image.tiffRepresentation,
      let rep = NSBitmapImageRep(data: tiff),
      let png = rep.representation(using: .png, properties: [:]) else {
    fatalError("failed to render icon")
}
try! png.write(to: URL(fileURLWithPath: "dist/icon_1024.png"))
print("wrote dist/icon_1024.png")
```

- [ ] **Step 2: Generate the .icns**

Run from repo root:

```bash
mkdir -p dist/Zenith.iconset Zenith/Resources
swift scripts/make_icon.swift
for s in 16 32 128 256 512; do
  sips -z $s $s dist/icon_1024.png --out dist/Zenith.iconset/icon_${s}x${s}.png >/dev/null
  sips -z $((s*2)) $((s*2)) dist/icon_1024.png --out dist/Zenith.iconset/icon_${s}x${s}@2x.png >/dev/null
done
iconutil -c icns dist/Zenith.iconset -o Zenith/Resources/Zenith.icns
```

Expected: `Zenith/Resources/Zenith.icns` exists, `file` reports "Mac OS X icon".

- [ ] **Step 3: Add `dist/` to `.gitignore`**

Append line `dist/` to `.gitignore`.

- [ ] **Step 4: Commit**

```bash
git add scripts/make_icon.swift Zenith/Resources/Zenith.icns .gitignore
git commit -m "feat: add app icon and icon generation script"
```

---

### Task 4: Bundle assembly, install, DMG

**Files:**
- Create: `Zenith/Resources/Info.plist`
- Modify: `Makefile` (add `app`, `install`, `dmg` targets + `.PHONY`)

**Context:** `make app` assembles `dist/Zenith.app/Contents/{MacOS/Zenith, Info.plist, Resources/Zenith.icns}` from the release build and ad-hoc codesigns it. `make install` replaces `/Applications/Zenith.app` via `ditto`. `make dmg` builds `dist/Zenith-0.1.0.dmg` from a staging folder containing the app and an `/Applications` symlink. Swift release binary lives at `Zenith/.build/release/Zenith`.

- [ ] **Step 1: Write `Zenith/Resources/Info.plist`**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleIdentifier</key>
	<string>io.github.Gkyohd.zenith</string>
	<key>CFBundleName</key>
	<string>Zenith</string>
	<key>CFBundleDisplayName</key>
	<string>Zenith</string>
	<key>CFBundleExecutable</key>
	<string>Zenith</string>
	<key>CFBundleIconFile</key>
	<string>Zenith</string>
	<key>CFBundlePackageType</key>
	<string>APPL</string>
	<key>CFBundleShortVersionString</key>
	<string>0.1.0</string>
	<key>CFBundleVersion</key>
	<string>0.1.0</string>
	<key>LSMinimumSystemVersion</key>
	<string>14.0</string>
	<key>NSHighResolutionCapable</key>
	<true/>
	<key>NSPrincipalClass</key>
	<string>NSApplication</string>
</dict>
</plist>
```

- [ ] **Step 2: Add packaging targets to the Makefile**

Change the first line to:

```make
.PHONY: build run clean release check app install dmg
```

Append at the end of the Makefile:

```make
APP_NAME = Zenith
VERSION = 0.1.0
DIST = dist
APP = $(DIST)/$(APP_NAME).app

app: release
	rm -rf $(APP)
	mkdir -p $(APP)/Contents/MacOS $(APP)/Contents/Resources
	cp Zenith/.build/release/$(APP_NAME) $(APP)/Contents/MacOS/$(APP_NAME)
	cp Zenith/Resources/Info.plist $(APP)/Contents/Info.plist
	cp Zenith/Resources/Zenith.icns $(APP)/Contents/Resources/Zenith.icns
	codesign --force --deep -s - $(APP)
	@echo "App bundle: $(APP)"

install: app
	rm -rf /Applications/$(APP_NAME).app
	ditto $(APP) /Applications/$(APP_NAME).app
	@echo "Installed /Applications/$(APP_NAME).app"

dmg: app
	rm -rf $(DIST)/dmg-staging $(DIST)/$(APP_NAME)-$(VERSION).dmg
	mkdir -p $(DIST)/dmg-staging
	ditto $(APP) $(DIST)/dmg-staging/$(APP_NAME).app
	ln -s /Applications $(DIST)/dmg-staging/Applications
	hdiutil create -volname $(APP_NAME) -srcfolder $(DIST)/dmg-staging -ov -format UDZO $(DIST)/$(APP_NAME)-$(VERSION).dmg
	rm -rf $(DIST)/dmg-staging
	@echo "DMG: $(DIST)/$(APP_NAME)-$(VERSION).dmg"
```

(Makefile recipes MUST be indented with tabs, not spaces.)

- [ ] **Step 3: Build and verify the bundle**

Run: `make app`
Expected: `dist/Zenith.app` exists with `Contents/MacOS/Zenith`, `Contents/Info.plist`, `Contents/Resources/Zenith.icns`.

Run: `codesign --verify --verbose dist/Zenith.app`
Expected: `valid on disk` + `satisfies its Designated Requirement`.

- [ ] **Step 4: Verify DMG builds**

Run: `make dmg`
Expected: `dist/Zenith-0.1.0.dmg` created; `hdiutil verify dist/Zenith-0.1.0.dmg` passes.

- [ ] **Step 5: Commit**

```bash
git add Zenith/Resources/Info.plist Makefile
git commit -m "feat: add app bundle, install and dmg targets"
```

---

### Manual GUI verification (controller + user, after all tasks)

1. `make install`
2. `open /Applications/Zenith.app` → window appears, `pwd` prints `/Users/<user>`, typing/scroll/AI panel/autosuggest work.
3. `ps -ef | grep Zenith.app` shows the process running from /Applications.
4. User confirms Dock/Finder icon looks right and double-click launch works.
