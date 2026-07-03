.PHONY: build run clean release check app install dmg

build:
	cargo build --release
	@echo "Rust build complete"
	cd Zenith && swift build
	@echo "Swift build complete"

release:
	cargo build --release
	cd Zenith && swift build -c release

run: build
	cd Zenith && swift run

clean:
	cargo clean
	cd Zenith && swift package clean

check:
	cargo test --workspace
	cargo clippy --workspace -- -D warnings

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
