# Sivgo — common development commands
# Install just: cargo install just

# `just` defaults to `sh` for recipe execution unless a Windows shell is set.
# On Windows, use `cmd.exe` so the built-in recipes work without Git Bash/MSYS.
set windows-shell := ["cmd.exe", "/C"]

# Default recipe: list available commands
default:
    @just --list

# ─── Build ────────────────────────────────────────────────────────────

# Build all crates in the workspace
build:
    cargo build --workspace

# Build in release mode
build-release:
    cargo build --workspace --release

# Check all crates (fast compile check, no codegen)
check:
    cargo check --workspace

# ─── Test ─────────────────────────────────────────────────────────────

# Run all tests
test:
    cargo test --workspace

# Run tests for a specific crate (e.g. just test-crate sivgo-math)
test-crate crate:
    cargo test -p {{crate}}

# Run tests with output shown
test-verbose:
    cargo test --workspace -- --nocapture

# ─── Lint & Format ───────────────────────────────────────────────────

# Run clippy on the whole workspace
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Check formatting
fmt-check:
    cargo fmt --all -- --check

# Auto-format all code
fmt:
    cargo fmt --all

# Full lint pass (clippy + format check)
lint: clippy fmt-check

# Check Cargo.lock against the RustSec advisory database
audit:
    cargo audit

# ─── Run ──────────────────────────────────────────────────────────────

# Run the desktop application
desktop:
    cargo run -p sivgo-desktop

# Run the desktop application in release mode
desktop-release:
    cargo run -p sivgo-desktop --release

# Run the web/Salvo backend server
serve:
    cargo run -p sivgo-web

# Run the web server in release mode
serve-release:
    cargo run -p sivgo-web --release

# Render an SVG file to PNG via CLI (e.g. just render input.svg output.png)
render input output:
    cargo run -p sivgo-desktop -- render {{input}} {{output}}

# Show SVG file info via CLI
info input:
    cargo run -p sivgo-desktop -- info {{input}}

# ─── Docs ─────────────────────────────────────────────────────────────

# Generate API documentation
doc:
    cargo doc --workspace --no-deps

# Generate and open API documentation in browser
doc-open:
    cargo doc --workspace --no-deps --open

# ─── Benchmark & Fuzz ────────────────────────────────────────────────

# Run benchmarks
bench:
    cargo bench --workspace

# Run fuzz tests (requires cargo-fuzz: cargo install cargo-fuzz)
fuzz target="svg_parse" duration="60":
    cd fuzz && cargo +nightly fuzz run {{target}} -- -max_total_time={{duration}}

# ─── Clean ────────────────────────────────────────────────────────────

# Remove build artifacts
clean:
    cargo clean

# ─── CI ───────────────────────────────────────────────────────────────

# Run the full CI check locally (build + test + lint)
ci: check lint test

# Run full CI with release build
ci-full: build-release lint test bench
