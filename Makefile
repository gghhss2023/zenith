.PHONY: build run clean release

build:
	cargo build
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
