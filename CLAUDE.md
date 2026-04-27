# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Rust/egui (eframe) desktop + WASM app called `digital_garden`. A multi-window "garden" of mini-apps (calculator, fractal clock, binary clock, canvas viewer, projects, workouts, collections) plus the main Digital Garden markdown notes browser. Crate name in `Cargo.toml` is `digital_garden`; the public type exposed from `lib.rs` is `TemplateApp`.

## Common commands

- `cargo run` — native dev build
- `cargo check` / `cargo clippy --all-targets --all-features -- -D warnings` — fast feedback
- `cargo fmt --all`
- `cargo test --workspace --all-targets --all-features` — run all tests; single test: `cargo test <test_name>`
- `./check.sh` — full CI-equivalent: check (native + wasm), fmt, clippy, tests, doc tests, `trunk build`
- `trunk serve` / `trunk build` — WASM build (uses `index.html`, `Trunk.toml`)

WASM target is `wasm32-unknown-unknown`; web entry is `src/main.rs` + `index.html`.

## Architecture

Entry: `src/lib.rs` exposes `TemplateApp` (in `src/app.rs`) which is the single eframe `App`. State persistence is via `serde` + eframe `persistence` feature — fields marked `#[serde(skip)]` are runtime-only; user preferences and persisted file paths (notes dir, workouts.json, last canvas) survive restarts.

Two top-level module groups:

- `src/apps/` — independent mini-apps (calculator, fractal_clock, binary_clock, canvas_view, projects, workouts, collections, easy_mark). Each is owned by `TemplateApp` and toggled via `*_is_open` bool fields. `easy_mark` is a vendored markdown-ish renderer.
- `src/digital_garden/` — the main notes app. Internal pieces: `note_directory` (filesystem load), `note` (per-note state), `markdown_parser` (also re-used by `canvas_view` for rich text in canvas nodes — hence `pub(crate)`), `search`, `sidebar`, `graph_view`, `history`, `watcher` (filesystem hot-reload via `notify`), `theme`.

Two project-wide traits in `lib.rs`: `View` (renders into a `Ui`) and `AppWindow` (manages its own `Window` with open/close state keyed by `name() -> &'static str`).

Platform-conditional deps in `Cargo.toml`: native uses `rfd` (file dialogs), `notify` (fs watcher), `tracing-subscriber`; wasm uses `web-sys`, `wasm-bindgen*`, and `getrandom` with the `js` backend. Hot-reload / file dialogs are native-only — guard with `cfg(not(target_arch = "wasm32"))` when adding similar features.

## Conventions

- No emojis or em-dashes in code/comments. Keep comments concise and evergreen (no "new"/"improved"/"enhanced" naming).
- Don't remove existing comments unless provably false.
- Surgical edits only — match existing style; don't refactor adjacent code.
