# Tic-Tac-Toe — Design Document

A player-vs-computer Tic-Tac-Toe desktop application, built as the first
instance of a reusable application template: a modern HTML/CSS UI directing
heavy computation in Rust, as OS-independent as practical.

This document records the goals, the framework decision, the general
application template, and how this specific app instantiates it.

---

## 1. Goals and Context

The application is one of a planned **collection of independent applications**
sharing a common shape:

- **Computationally intensive core.** The eventual applications include AI
  players for games such as poker, chess, go, dominoes, and backgammon, as well
  as non-game applications. The compute side must run at native speed with
  headroom for deep search.
- **Modern, polished UI.** HTML/CSS front end.
- **OS independence.** Windows, macOS, and Linux from a single codebase.
- **Each app is independent.** There is no shared workspace or monorepo across
  apps; instead there is a **design template** each app follows.

## 2. Framework Decision

**Chosen: Tauri v2** — a Rust backend hosting a native OS window with an
embedded webview running the HTML/CSS/JS front end.

Rationale:

- Matches the desired shape exactly: HTML/CSS frontend over a Rust backend.
- Uses the OS's built-in webview (WebView2 on Windows, WKWebView on macOS,
  WebKitGTK on Linux) rather than bundling a browser — binaries are a few MB
  instead of 100+ MB, and memory use is lower than Electron.
- The compute-heavy code is ordinary native Rust, not a JS addon.
- Ships to all three desktop OSes from one codebase; Tauri v2 can also target
  mobile if ever needed.

Alternatives considered:

| Option | Why not chosen |
|---|---|
| Electron | Bundles Chromium: much larger binaries and RAM; compute must go through Node addons or sidecars. |
| Qt / PySide6 | Mature and fast, but native-toolkit styling takes more effort to polish, and it doesn't use HTML/CSS. |
| egui / Slint / iced / Dioxus | Rust-native UI models or Rust-authored markup — do not match the HTML/CSS preference. |
| Pure web + WASM | Considered attractive (zero install), but a desktop app was desired; note the same core could still compile to WASM later because it is UI-agnostic. |

Known tradeoff: rendering happens on three slightly different web engines, so
occasional minor CSS differences across platforms are possible. Acceptable for
application UIs.

## 3. The Application Template

Each application is an independent, self-contained Tauri project with the same
three-layer skeleton:

```
my-app/
├── Cargo.toml            # Rust workspace: src-core + src-tauri
├── src-core/             # LAYER 1: pure compute. No Tauri, no UI, no I/O assumptions.
├── src-tauri/            # LAYER 2: thin shell — window, commands, worker, events.
│   ├── src/main.rs
│   ├── tauri.conf.json
│   ├── capabilities/     # locked-down permission set
│   └── icons/
├── src/                  # LAYER 3: HTML/CSS/TS front end
│   ├── api.ts            #   the ONLY door to the Rust backend
│   ├── main.ts           #   UI controller
│   └── styles.css
├── index.html
└── package.json          # Vite + @tauri-apps/api + @tauri-apps/cli
```

### Layer 1 — Compute core (`src-core`)

A plain Rust library crate that knows nothing about Tauri, the webview, or any
UI. It exposes the computation as ordinary functions over ordinary types.

**The load-bearing rule:** the core has zero UI dependencies. This makes it
unit-testable (`cargo test -p <core>`), benchmarkable, runnable headless from a
CLI or server, and portable (including to WASM) without touching the expensive,
correctness-critical code.

For long-running computation, the template's core signature injects progress
and cancellation rather than referencing any UI:

```rust
pub fn solve(
    input: &Problem,
    params: &Params,
    cancel: &CancelFlag,                      // core polls this
    on_progress: &mut dyn FnMut(Progress),    // core reports through this
) -> Result<Solution, SolveError>;
```

### Layer 2 — Shell (`src-tauri`)

The thin adapter that owns the window, holds authoritative application state,
and exposes the core as a small set of Tauri commands. It contains no domain
rules and no algorithms.

The **UI-directs-computation contract** is a four-message shape:

1. `start(params) -> job_id` — async command; spawns work on a worker thread
   and returns immediately.
2. **progress events** — the core's progress callback forwards to
   `app.emit(...)`; partial results stream to the UI.
3. **result / error event** — emitted on completion.
4. `cancel(job_id)` — flips the cancellation flag the core polls.

The computation always runs off the UI thread (`spawn_blocking` or a dedicated
thread) so the window never freezes; extremely heavy or crash-prone engines can
instead run as a Tauri **sidecar process** that can be killed and restarted.

Security posture: `capabilities/` starts locked down (`core:default` only) and
opens up only as needed; CSP restricted to `'self'`.

### Layer 3 — Front end (`src/`)

Any web stack works; the template default is **vanilla TypeScript + Vite** for
minimal dependencies. Two conventions:

- **`api.ts` is the single boundary.** Nothing else imports `@tauri-apps/api`.
  If the protocol changes, exactly one file changes.
- The UI holds **no domain rules**. It renders snapshots returned by the
  backend and forwards user intent. The Rust side is the single source of
  truth for legality and outcomes — the UI is never trusted.

Wire types should be kept small and, in larger apps, generated from the Rust
types (`tauri-specta` / `ts-rs`) so the two sides cannot drift.

### Runtime model (how a Tauri app runs)

- The user launches a single native executable; Rust `main()` starts the Tauri
  runtime, which opens a native window with an embedded webview.
- In production the HTML/CSS/JS is compiled into the executable and served
  internally; there is no external web server.
- JS calls Rust via `invoke("command", args)`; Rust pushes to JS via
  `app.emit(event, payload)` + `listen(event, cb)`.
- Development: `pnpm tauri dev` (Vite dev server + hot reload + native
  window). Production: `pnpm tauri build` produces platform installers
  (`.msi`/`.exe`, `.dmg`/`.app`, `.deb`/`.AppImage`). Each platform's installer
  is built on (or targeting) that platform — typically via CI runners.

## 4. This Application: Tic-Tac-Toe

### 4.1 Overview

Player-vs-computer Tic-Tac-Toe. Before each game the player chooses their mark
(X or O) and who moves first. The computer plays perfectly via full-depth
alpha-beta minimax — it is unbeatable; the best achievable result against it is
a draw.

### 4.2 The AI: the `game-player` crate (git dependency)

The search comes from the external crate
[`game-player`](https://github.com/jambolo/game-player) (v0.4.0, edition 2024),
consumed by `src-core` as a **Cargo git dependency** on
`https://github.com/jambolo/game-player.git`, pinned with `rev` at commit
`0b8ce6f`.

Integration points implemented (all in `src-core`):

| `game-player` trait | Implemented by | Notes |
|---|---|---|
| `State` (assoc. `Action`) | `Board` / `Move` | `fingerprint()`, `whose_turn()`, `is_terminal()`, immutable `apply()` |
| `StaticEvaluator` | `Evaluator` | Scored **always from Alice's perspective** |
| `minimax::ResponseGenerator` | `Generator` | Delegates to `Board::legal_moves()` |
| `minimax::search(sef, rg, s0, max_depth)` | `best_move()` | Returns `Option<Move>` |

Crate conventions honored:

- **Alice = maximizer, Bob = minimizer.** Mapping chosen: **Alice → X,
  Bob → O**. The evaluator is never sign-flipped by turn — the crate documents
  that Alice-perspective scoring is load-bearing for its max/min logic.
- **Empty-generate-iff-terminal policy.** `search` never calls `is_terminal()`
  itself; an empty result from `generate` is its sole terminality signal.
  `Board::legal_moves()` returns `[]` exactly when the position is won or full,
  keeping `generate` and `is_terminal()` in exact agreement (the crate debug-
  asserts this).
- The search calls `state.apply(&action)` internally; `apply` returns a new
  state and leaves the original unchanged.

### 4.3 Engine design (`src-core`, crate `ttt-core`)

- `Mark` (X/O) with mappings to/from `PlayerId`.
- `Board`: `[Option<Mark>; 9]` plus `turn: PlayerId`. Copy-able immutable value
  type. Win detection over the 8 lines; `Status` = `InProgress | Win(Mark) |
  Draw`.
- **Fingerprint:** 2 bits per cell (empty/X/O) packed into a `u64`, plus the
  turn bit at bit 62 — deterministic, position-only, collision-free for this
  game (used to key the crate's transposition table).
- **Evaluator:** terminal X-win = `+100 − filled`, O-win = `−100 + filled`,
  else `0`. The `filled` adjustment makes the search prefer faster wins and
  slower losses. Values stay within `[bob_wins_value(), alice_wins_value()]`
  as required. Non-terminal `0` is sound because the search always reaches
  terminals (see depth).
- **Search depth:** `MAX_DEPTH = 9` — a game is at most 9 plies, so the full
  tree is explored and play is perfect.
- `Game` facade: board + the human's mark; validates human moves
  (`GameOver` / `OutOfBounds` / `Occupied`) and answers `is_human_turn()` /
  `ai_best()`. This is the authoritative game object the shell holds.

### 4.4 Shell design (`src-tauri`)

State: `AppState { game: Mutex<Option<Game>> }`, Tauri-managed. Rules are
enforced in Rust; the UI is never trusted.

Commands (the protocol):

| Command | Args | Behavior |
|---|---|---|
| `new_game` | `mark: char, humanStarts: bool` | Creates the game, returns a `GameView`. |
| `play` | `cell: usize` | Validates it is the human's turn and the move is legal; applies; returns `GameView`. |
| `ai_move` | — | Snapshots the board (not holding the lock across `await`), emits `ttt://thinking = true`, runs `best_move` via `spawn_blocking`, emits `false`, re-acquires the lock, **re-validates** (the game may have been reset during the search), applies, returns `GameView`. |

`GameView` (serde, camelCase): `cells: [Option<char>; 9]`, `turn`, `humanMark`,
`status` (tagged union `inProgress | win{winner} | draw`), `awaitingAi: bool`.

This is the template's start/compute/finish contract in miniature. Tic-Tac-Toe
search is effectively instant, so there is no `cancel` and the "job" is a
single awaited command — but the worker-thread + thinking-event shape is kept
deliberately so the skeleton scales to heavier games without restructuring.

Window: 480×660 (min 380×560), resizable. CSP `default-src 'self'`;
capabilities: `core:default` only.

### 4.5 Front end design (`src/`, vanilla TS + Vite)

- `api.ts` — the single backend boundary: `newGame`, `play`, `aiMove`,
  `onThinking`, plus the mirrored `GameView` types.
- `main.ts` — UI controller. Renders `GameView` snapshots; no rules of its own.
  Flow: click cell → `play` → render → if `awaitingAi`, call `aiMove` → render.
  A `busy` flag blocks input while the AI move is in flight; backend rejections
  (races) are logged and the next render corrects the UI. Status line shows
  turn / thinking / outcome; thinking hint driven by the `ttt://thinking`
  event.
- `index.html` — setup panel (your mark: X/O; who goes first: you/computer),
  status line (`aria-live`), 3×3 board (`role="grid"`).
- `styles.css` — dark slate theme; blue accent; X in light blue, O in yellow;
  hover/active affordances; cells disabled unless it is the human's turn and
  the cell is empty.

### 4.6 Testing and verification

Engine tests (`cargo test -p ttt-core`, 9 tests, all passing):

- Win detection across rows, columns, diagonals; no false positives.
- AI takes an immediate win; AI blocks an immediate loss.
- Optimal-vs-optimal self-play always draws (both X-first and O-first).
- **Exhaustive unbeatability proof:** every possible line of human play is
  explored (human plays *all* legal moves at each of their turns, computer
  replies with `best_move`), asserting the human never wins — run twice, with
  the human moving first and with the computer moving first.
- `apply` immutability and turn flip; terminal boards yield no best move.

Additional verification performed in this session: `cargo check` of the full
Tauri shell compiles cleanly; `cargo clippy` clean; `tsc --noEmit` clean; the
delivered archive was re-extracted fresh and the engine tests re-run.

### 4.7 Build and run

```bash
pnpm install
pnpm tauri dev      # development: Vite + hot reload + native window
pnpm tauri build    # production: platform installer
cargo test -p ttt-core # engine tests, no UI needed
```

Prerequisites: Rust stable (edition 2024; developed on rustc 1.99), Node ≥ 22.12,
and Tauri's per-OS system deps (on Windows just the WebView2 runtime; on Linux
the GTK/WebKitGTK dev packages).

Icons: PNG set + Windows `.ico` are bundled; regenerate a full set including
macOS `.icns` with `pnpm tauri icon src-tauri/icons/icon.png`.

## 5. Decision Log

| Decision | Choice | Why |
|---|---|---|
| UI/compute framework | Tauri v2 | HTML/CSS over Rust; small binaries; native compute; 3 OSes from one codebase |
| Per-app structure | Independent repo per app, common template | Apps are independent by requirement; the template, not a workspace, is what's shared |
| Core/UI separation | `src-core` has zero UI deps | Testable, benchmarkable, headless-capable, portable |
| AI library | `game-player` via Cargo git dependency | Existing minimax + transposition table; `rev` pins an exact upstream commit |
| Alice/Bob mapping | Alice → X, Bob → O | Fixes maximizer/minimizer to marks once; evaluator stays Alice-perspective |
| Search depth | 9 (full tree) | Perfect play; trivial cost for this game |
| Eval tie-break | ±(WIN − filled) | Prefer faster wins, slower losses |
| Frontend stack | Vanilla TS + Vite | Minimal deps; adequate for a 3×3 board; template default |
| Mark/first-move choice | In-app, per game | User-selected option |
| AI off the UI thread | `spawn_blocking` + thinking event | Template contract; scales to heavy games even though TTT is instant |
| Authoritative state | Rust `Mutex<Option<Game>>` | UI never trusted with rules; races re-validated after the search |
