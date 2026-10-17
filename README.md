# Tic-Tac-Toe (player vs. computer)

A small desktop app built on the **Tauri + Rust core + HTML/CSS front end**
template. The computer plays perfectly using the alpha-beta minimax search from
the [`game-player`](https://github.com/jambolo/game-player) crate, pulled in as
a Cargo git dependency.

You choose your mark (X or O) and who moves first before each game. The computer
is unbeatable — the best you can do is force a draw.

## Layout

```
tic-tac-toe/
├── Cargo.toml            # Rust workspace: src-core + src-tauri
├── src-core/             # UI-agnostic engine (rules + minimax adapter). No Tauri deps.
│   └── src/lib.rs
├── src-tauri/            # Thin Tauri shell: window, commands, worker, events.
│   ├── src/main.rs
│   ├── tauri.conf.json
│   ├── capabilities/
│   └── icons/
├── src/                  # Vanilla-TypeScript front end.
│   ├── api.ts            #   the only door to the Rust backend
│   ├── main.ts           #   UI controller
│   └── styles.css
├── index.html
└── package.json
```

The design keeps the computation (`src-core`) completely separate from the UI
shell (`src-tauri` + `src/`), so the engine can be tested headless and the same
skeleton can be reused for other apps.

## Prerequisites

* **Rust** (stable, edition 2024 — rustc ≥ 1.85; developed against 1.99).
* **Node.js** (≥ 22.12) and **pnpm** (`npm install -g pnpm`).
* **Tauri system dependencies** for your OS — see
  <https://tauri.app/start/prerequisites/>. On Windows this is just the WebView2
  runtime (already present on Windows 10/11).

## Run in development

```bash
pnpm install
pnpm tauri dev
```

This launches the app window with the Vite dev server and hot reload.

## Build a distributable

```bash
pnpm tauri build
```

Produces a native installer for the platform you build on (`.msi`/`.exe` on
Windows, `.dmg`/`.app` on macOS, `.deb`/`.AppImage` on Linux).

> Note: the bundled icons include PNG and a Windows `.ico`. For a macOS release
> build you'll also want an `.icns` — regenerate the full set from a source image
> with `pnpm tauri icon src-tauri/icons/icon.png`.

## Test the engine

The AI logic is verified independently of the UI:

```bash
cargo test -p ttt-core
```

The tests include an exhaustive proof that the computer never loses from any line
of human play (both when the human moves first and when the computer does), plus
optimal-vs-optimal always drawing, and correct blocking/winning tactics.

## How it fits the template

* **`src-core`** implements `game-player`'s `State`, `StaticEvaluator`, and
  `minimax::ResponseGenerator` traits for Tic-Tac-Toe, then calls
  `minimax::search`. Alice→X (maximizer), Bob→O (minimizer).
* **`src-tauri`** holds the authoritative `Game` in managed state, exposes
  `new_game`, `play`, and `ai_move` commands, and runs the search on a worker
  thread (`spawn_blocking`) with a `ttt://thinking` event bracketing it — the
  template's "UI directs computation" contract. For Tic-Tac-Toe the search is
  instant, but the shape scales to heavier games.
* **`src/api.ts`** is the sole boundary to the backend; the rest of the UI never
  touches Tauri APIs directly.
