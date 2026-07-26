# Development Guide

How to set up a development environment for this project, debug the Rust and
TypeScript sides in VS Code, run both test suites, and build/deploy a release.

Written for **VS Code on Linux** (the primary target) and cross-checked for
**VS Code on Windows using a Git for Windows ("Git Bash") shell**. Every shell
command below is bash syntax and works unmodified in both. Anywhere the two
platforms genuinely differ, it's called out explicitly.

---

## 1. Environment Setup

### 1.1 Install the toolchain

You need Rust, Node.js, and (on Linux) the system webview libraries Tauri
links against.

**Linux (Debian/Ubuntu-based; adjust package names for other distros):**

```bash
# Rust, via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Node.js (LTS) — via your distro, nvm, or nodejs.org. Example with apt:
sudo apt-get update
sudo apt-get install -y nodejs npm

# pnpm (the package manager this project uses)
npm install -g pnpm

# Tauri's Linux system dependencies (GTK/WebKitGTK + friends)
sudo apt-get install -y \
  libgtk-3-dev libwebkit2gtk-4.1-dev libsoup-3.0-dev \
  libjavascriptcoregtk-4.1-dev librsvg2-dev libayatana-appindicator3-dev
```

**Windows (VS Code + Git Bash):**

1. Install **Rust** via [rustup](https://rustup.rs) (the Windows installer).
   Accept the default host triple (`x86_64-pc-windows-msvc`).
2. Install **Visual Studio Build Tools** with the "Desktop development with
   C++" workload — Tauri's Windows target needs the MSVC linker, not just
   rustup. (If you already have Visual Studio installed, the same workload
   works.)
3. Install **Node.js** (LTS) from [nodejs.org](https://nodejs.org) — the
   installer puts `node`/`npm` on `PATH`, which Git Bash inherits. Then
   install **pnpm** (the package manager this project uses):
   `npm install -g pnpm`.
4. Install **Git for Windows** (gives you the Git Bash shell) from
   [git-scm.com](https://git-scm.com).
5. The **WebView2 Runtime** is preinstalled on current Windows 10/11; if
   `tauri dev` complains it's missing, get it from Microsoft's
   [WebView2 downloads page](https://developer.microsoft.com/microsoft-edge/webview2/).

Once installed, `rustc`, `cargo`, `node`, `pnpm`, and `git` are all plain
Windows executables on `PATH`, so they behave identically whether invoked from
Git Bash, PowerShell, or cmd — every command in this document works as-is in
Git Bash.

**Windows/Git Bash gotchas worth knowing up front:**

- **Path length.** Rust's `target/` directories nest deeply and can hit
  Windows' historical 260-character path limit. If you see build errors
  mentioning path length, enable long paths for Git (`git config --global
  core.longpaths true`) and, if needed, Windows long-path support (Group
  Policy or the registry key documented by Microsoft), and keep the project
  near the drive root (e.g. `C:\dev\tic-tac-toe`) rather than deeply nested.
- **Line endings.** Git for Windows defaults to converting `LF` → `CRLF` on
  checkout. That's harmless for Rust/TS tooling here, but for a cleaner diff
  history run `git config --global core.autocrlf input` once, or rely on the
  workspace's `.vscode/settings.json` (`"files.eol": "\n"`), which makes VS
  Code write `LF` for files you edit.
- **MSYS path mangling.** Git Bash auto-converts arguments that look like
  Unix paths (e.g. a leading `/`) into Windows paths, which occasionally
  trips up tools that expect a literal string starting with `/`. None of the
  commands in this guide hit that case; if you add scripts that do, prefix
  the invocation with `MSYS_NO_PATHCONV=1`.

### 1.2 Install the Tauri CLI and project dependencies

The Tauri CLI is a dev dependency of this project (invoked via `pnpm
tauri`), so a global install isn't required — just:

```bash
git clone <this-repo-url> tic-tac-toe
cd tic-tac-toe
git submodule update --init --recursive   # fetches vendor/game-player
pnpm install
```

Sanity-check both halves build:

```bash
cargo check --workspace   # Rust: src-core + src-tauri
pnpm typecheck         # TypeScript: tsc --noEmit
```

### 1.3 VS Code setup

Open the repository root (`tic-tac-toe/`) as the VS Code workspace folder —
not a subfolder — so the checked-in `.vscode/` configuration applies.

**Recommended extensions** (VS Code will prompt you to install these; they're
listed in `.vscode/extensions.json`):

| Extension | ID | Purpose |
|---|---|---|
| rust-analyzer | `rust-lang.rust-analyzer` | Rust language server: inline errors, go-to-def, inline "Run"/"Debug" codelens on tests |
| CodeLLDB | `vadimcn.vscode-lldb` | The debugger rust-analyzer's codelens uses; also what the checked-in `launch.json` Rust configs use |
| Tauri | `tauri-apps.tauri-vscode` | `tauri.conf.json`/capabilities schema support, command palette shortcuts |
| Vitest | `vitest.explorer` | Test explorer sidebar for the TypeScript test suite |
| Prettier | `esbenp.prettier-vscode` | Formats `.ts`/`.css`/`.html` on save |
| ESLint | `dbaeumer.vscode-eslint` | Optional; no ESLint config is checked in yet, but the extension is ready if you add one |

The checked-in `.vscode/settings.json` already:

- points rust-analyzer at the workspace `Cargo.toml` (so it resolves both
  `src-core` and `src-tauri`, plus the `vendor/game-player` path dependency),
- runs `cargo clippy` (not just `cargo check`) for inline diagnostics,
- pins the TypeScript version to the one in `node_modules` rather than VS
  Code's bundled copy,
- sets Git Bash as the default integrated terminal **on Windows only** (no
  effect on Linux),
- format-on-save for `.rs` (via rust-analyzer) and `.ts` (via Prettier).

---

## 2. Running the App in Development

```bash
pnpm tauri dev
```

This starts the Vite dev server, waits for it, compiles the Rust shell, and
opens the native window with hot module reload on the frontend. There's also
a VS Code task for this: **Terminal → Run Task… → `tauri: dev`**.

---

## 3. Running Tests

### 3.1 Rust — `ttt-core` (the engine)

The compute core is a plain library crate with no Tauri/UI dependencies, so
its tests run instantly and need no display:

```bash
cargo test -p ttt-core
```

VS Code task: **Terminal → Run Task… → `rust: test core`** (also the default
test task — `Ctrl+Shift+B`-adjacent `Tasks: Run Test Task`).

What's covered (9 tests): win detection across every line, the AI taking an
immediate win and blocking an immediate loss, optimal-vs-optimal self-play
always drawing, an **exhaustive** proof the computer never loses from any line
of human play (checked with the human moving first and with the computer
moving first), and `apply()`'s immutability/turn-flip contract. See
`src-core/src/lib.rs`'s `#[cfg(test)] mod tests` for the full list.

Also worth running regularly:

```bash
cargo clippy --workspace --all-targets   # lints, including the Tauri shell
cargo fmt --all                          # formats in place
cargo fmt --all -- --check               # CI-style check without modifying files
```

`src-tauri` itself (the Tauri shell binary) has no separate `cargo test`
suite — its command handlers are thin wrappers over `ttt-core`, which is
where the logic and the tests live. If you add non-trivial logic directly in
`src-tauri`, add `#[cfg(test)]` tests there the same way.

### 3.2 TypeScript — the frontend

Tests run with [Vitest](https://vitest.dev) against a `jsdom` environment,
configured in `vite.config.ts`'s `test` block:

```bash
pnpm test          # run once (vitest run)
pnpm test:watch  # re-run on file changes
```

VS Code: the Vitest extension adds a **Testing** sidebar (flask icon) that
discovers and runs/debugs individual tests with a click, using the same
`vite.config.ts`. Alternatively, **Terminal → Run Task… → `ts: test`**.

Current coverage: `src/status.test.ts` exercises `statusText()`
(`src/status.ts`), the pure function that turns a `GameView` snapshot into the
status line's text — every branch (human's turn, AI thinking vs. not yet
started, human win, computer win, draw). This logic was deliberately factored
out of `main.ts` (which is otherwise DOM-driven glue) specifically so it has
something meaningful to unit test; as the UI grows, prefer pulling more pure
logic out into small modules next to their own `*.test.ts` file rather than
testing through the DOM.

Type-checking is a separate, complementary check (Vitest doesn't type-check
by itself):

```bash
pnpm typecheck   # tsc --noEmit
```

`pnpm build` runs `typecheck` and then the production Vite build, so a
type error fails the build even if no test exercises that code path.

### 3.3 Running everything

```bash
cargo test -p ttt-core && cargo clippy --workspace --all-targets && pnpm test && pnpm typecheck
```

---

## 4. Debugging

### 4.1 Debugging Rust

**Unit tests (the easy, everyday path).** With rust-analyzer + CodeLLDB
installed, open `src-core/src/lib.rs`, set a breakpoint inside any test or the
code it calls, and click the **▶ Debug** codelens that appears above the
`#[test] fn` (or above `mod tests` to debug all of them). rust-analyzer
generates the launch config on the fly.

The same thing is also available as an explicit, repeatable launch
configuration — **Run and Debug → "Rust: Debug ttt-core tests"** — for when
you want it in `launch.json` rather than hunting for the codelens (e.g. to
debug a specific failure without scrolling to it first; edit the `args` array
in `.vscode/launch.json` to target a single test by name, e.g. `["--",
"tests::ai_blocks_the_immediate_loss"]`).

**The running Tauri app (backend commands in `src-tauri/src/main.rs`).** The
robust way to debug the shell's `new_game`/`play`/`ai_move` command handlers
is to start the app normally and then attach:

1. Launch the app: **Terminal → Run Task… → `tauri: dev`** (or `pnpm tauri
   dev` in a terminal).
2. Set your breakpoints in `src-tauri/src/main.rs`.
3. **Run and Debug → "Rust: Attach to running Tauri app"**, then pick the
   `tic-tac-toe` process from the list VS Code shows.
4. Trigger the code path from the UI (click a cell, start a new game) — the
   breakpoint hits like any other debugger.

Attaching (rather than having VS Code launch the binary itself) sidesteps any
question of how the binary discovers the Vite dev server — it's launched
exactly the way the Tauri CLI always launches it; the debugger just attaches
afterward. CodeLLDB works the same way on Linux and on Windows (against the
MSVC-built binary), so this workflow is identical on both platforms.

### 4.2 Debugging TypeScript

Tauri's webview is a real browser engine, but *which* one differs by OS
(WebKitGTK on Linux, WebView2/Chromium on Windows), which affects which VS
Code debugging path is available. Three options, from "always works" to
"full VS Code integration":

**1. Built-in devtools (always works, both platforms).** Run `pnpm tauri
dev`, then right-click anywhere in the app window → **Inspect Element**.
Tauri enables devtools automatically in debug builds. You get full
breakpoints, console, network tab, and source maps (Vite serves them in dev)
— it's just the webview's own devtools rather than VS Code's editor. This is
the simplest option and the one to reach for first.

**2. Pure frontend logic in a real Chrome/Edge tab (VS Code breakpoints,
both platforms).** For UI/rendering logic that doesn't depend on the Tauri
backend:

```bash
pnpm dev   # Vite only, no Tauri
```

Then **Run and Debug → "TS: Debug frontend in Chrome (Vite only, no
backend)"**. This launches Chrome pointed at `http://localhost:5173` with VS
Code as the debugger — real breakpoints in `.ts` source, set directly in the
editor. The caveat: `api.ts`'s `invoke()` calls will reject, since there's no
Tauri backend listening on this path — this mode is for layout, rendering,
and pure-logic work (like `status.ts`), not full click-through flows.

**3. VS Code attached to the real embedded webview (full integration,
Windows only).** WebView2 is Chromium-based and speaks the Chrome DevTools
Protocol, so you can attach VS Code's JS debugger directly to the actual
running app window, `invoke()` calls and all:

```bash
# In Git Bash, before launching the app:
export WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9222"
pnpm tauri dev
```

Then **Run and Debug → "TS: Attach to Tauri WebView2 (Windows)"**. This gives
real breakpoints in `main.ts`/`api.ts` while driving the actual app UI.

WebKitGTK (Linux) does not speak the Chrome DevTools Protocol, so this
particular attach workflow is Windows-only; on Linux, option 1 (the built-in
inspector) is the equivalent full-integration debugging path.

---

## 5. Build and Deploy

### 5.1 Building a release bundle

```bash
pnpm tauri build
```

This runs `tsc --noEmit && vite build` (via `beforeBuildCommand` in
`tauri.conf.json`) to produce the optimized frontend in `dist/`, then compiles
the Rust shell in release mode and bundles the result into native installers.
There's also a VS Code task: **Terminal → Run Task… → `build: release
bundle`**.

Output lands under `src-tauri/target/release/bundle/`, in a subfolder per
installer type. Which types you get depends on the platform you build **on**:

| Built on | Produces |
|---|---|
| Linux | `.deb` (Debian/Ubuntu), `.AppImage` (portable, most distros), and `.rpm` if `rpmbuild` is installed |
| Windows | `.msi` (WiX) and `.exe` (NSIS) installers |
| macOS | `.app` bundle and `.dmg` |

**Cross-platform packaging.** Tauri does not cross-compile installers from
one OS to another out of the box — a `.msi` is built by running `tauri build`
*on* Windows, a `.dmg` *on* macOS, and so on. To produce all three from one
place, the standard approach is CI: a GitHub Actions workflow (or equivalent)
with one job per OS runner, each running `pnpm install --frozen-lockfile && pnpm tauri build` and
uploading `src-tauri/target/release/bundle/**` as an artifact. `tauri-action`
(the official GitHub Action) wraps exactly this.

**Unsigned builds.** By default the produced binaries are unsigned. That's
fine for local testing and internal distribution, but macOS Gatekeeper and
Windows SmartScreen will both warn on an unsigned app for end users — code
signing (an Apple Developer ID for macOS, an Authenticode certificate for
Windows) is a separate setup step to do before wider distribution; see
[Tauri's signing docs](https://tauri.app/distribute/) when that becomes
relevant.

**Icons.** The bundled icon set (`src-tauri/icons/`) currently includes PNGs
and a Windows `.ico`, sufficient for a Linux/Windows build. Before a macOS
release build, regenerate the full set (adds the `.icns` macOS needs) from a
source image:

```bash
pnpm tauri icon src-tauri/icons/icon.png
```

### 5.2 Running the release build locally

The compiled binary under `src-tauri/target/release/` (not inside `bundle/`)
runs standalone without an installer — useful for a quick smoke test of the
release build:

```bash
# Linux
./src-tauri/target/release/tic-tac-toe

# Windows (Git Bash)
./src-tauri/target/release/tic-tac-toe.exe
```

### 5.3 Versioning

Bump the version in **both** `package.json` and `src-tauri/tauri.conf.json`
(the `version` field) together — Tauri reads its version from the latter, and
they're expected to stay in sync. `Cargo.toml` version fields (`src-core`,
`src-tauri`) are independent Rust crate versions and don't need to match the
app version, though keeping them aligned avoids confusion.

---

## 6. Quick Reference

```bash
# Setup
git submodule update --init --recursive
pnpm install

# Develop
pnpm tauri dev

# Test
cargo test -p ttt-core
pnpm test
pnpm typecheck

# Lint / format (Rust)
cargo clippy --workspace --all-targets
cargo fmt --all

# Build a release bundle
pnpm tauri build
```
