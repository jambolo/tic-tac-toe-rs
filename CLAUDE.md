# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Player-vs-computer Tic-Tac-Toe desktop app: Tauri 2 shell + Rust engine + vanilla-TypeScript frontend. The AI plays perfectly via alpha-beta minimax from the vendored `game-player` crate (git submodule at `vendor/game-player`).

## Commands

```bash
# One-time setup
git submodule update --init --recursive   # fetches vendor/game-player
pnpm install

# Develop (Vite dev server + native window, HMR)
pnpm tauri dev

# Rust tests (engine — the meaningful test suite)
cargo test -p ttt-core
cargo test -p ttt-core -- tests::ai_blocks_the_immediate_loss   # single test

# TypeScript tests (Vitest + jsdom, configured in vite.config.ts)
pnpm test             # vitest run
pnpm test:watch
pnpm vitest run src/status.test.ts   # single file

# Lint / format / typecheck
cargo clippy --workspace --all-targets
cargo fmt --all
pnpm typecheck        # tsc --noEmit; also runs as part of pnpm build

# Release bundle (output: src-tauri/target/release/bundle/)
pnpm tauri build
```

## Architecture

Three strictly separated layers; keep them that way:

* **`src-core/` (`ttt-core` crate)** — UI-agnostic engine: game rules plus the minimax adapter. Implements `game-player`'s `State`, `StaticEvaluator`, and `minimax::ResponseGenerator` traits, then calls `minimax::search`. Player convention: Alice→X (maximizer), Bob→O (minimizer); the evaluator is always scored from Alice's perspective. No Tauri dependencies — tests run headless. All game logic and its tests belong here, not in the shell.

* **`src-tauri/` (thin Tauri shell)** — owns the window and the authoritative `Game` in Tauri-managed state (rules enforced in Rust, never trusted from the UI). Exposes three commands: `new_game`, `play`, `ai_move`. Translates engine types into the small JSON protocol (`GameView`). Runs the AI search on a worker thread (`spawn_blocking`) bracketed by the `ttt://thinking` event — the template's "UI directs computation" contract. No separate test suite; if non-trivial logic lands here, add `#[cfg(test)]` tests in place.

* **`src/` (frontend)** — `api.ts` is the **only** module allowed to import `@tauri-apps/api`; everything else goes through it. `main.ts` is DOM glue. Pure logic is factored into small modules (e.g. `status.ts`) with a sibling `*.test.ts` — prefer extending that pattern over testing through the DOM.

Rust workspace (`Cargo.toml`) has members `src-core` + `src-tauri`; `vendor/` is excluded and consumed as a path dependency.

## Notes

* Versioning: bump `package.json` and `src-tauri/tauri.conf.json` `version` fields together.
* `DEVELOPMENT.md` has the full VS Code debugging setup (CodeLLDB attach for Rust, WebView2 attach for TS on Windows).
* `vendor/game-player` has its own CLAUDE.md; treat the submodule as read-only from this repo.
