//! # Tauri shell — the thin adapter layer
//!
//! This binary is the *shell* of the template: it owns the window, holds the
//! authoritative game state, and exposes the compute core ([`ttt_core`]) to the
//! web UI as a small set of commands. It contains no game rules and no AI — those
//! live in `ttt-core`. Its only jobs are:
//!
//! * keep the authoritative [`Game`] in Tauri-managed state (so rules are
//!   enforced in Rust, not trusted from the UI),
//! * translate between the engine's types and a small JSON **protocol**
//!   ([`GameView`]) the front end renders,
//! * run the AI search off the UI thread and emit a "thinking" event around it —
//!   the template's start/compute/finish contract. Tic-Tac-Toe search is
//!   effectively instant, but doing it on a worker thread keeps the pattern
//!   correct for heavier games built from this same skeleton.

// Hide the console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Mutex;

use serde::Serialize;
use tauri::{Emitter, State};
use ttt_core::{Game, Mark, Move, Status};

/// Event channel the UI listens on to show/hide a "computer is thinking" hint.
const THINKING_EVENT: &str = "ttt://thinking";

/// Tauri-managed application state: the single authoritative game (if any).
struct AppState {
    game: Mutex<Option<Game>>,
}

// ---------------------------------------------------------------------------
// Protocol — the JSON shapes the front end consumes. Kept deliberately small.
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum StatusView {
    InProgress,
    Win { winner: char },
    Draw,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GameView {
    /// Nine cells: `'X'`, `'O'`, or `null`.
    cells: [Option<char>; 9],
    /// Mark of the player to move.
    turn: char,
    /// The human's mark this game.
    human_mark: char,
    /// Outcome.
    status: StatusView,
    /// True when the game is ongoing and it is the computer's turn.
    awaiting_ai: bool,
}

fn mark_char(m: Mark) -> char {
    match m {
        Mark::X => 'X',
        Mark::O => 'O',
    }
}

fn parse_mark(c: char) -> Result<Mark, String> {
    match c.to_ascii_uppercase() {
        'X' => Ok(Mark::X),
        'O' => Ok(Mark::O),
        other => Err(format!("invalid mark: {other:?}")),
    }
}

/// Build the wire view from the authoritative game.
fn view(game: &Game) -> GameView {
    let cells = game.board.cells().map(|c| c.map(mark_char));
    let status = match game.status() {
        Status::InProgress => StatusView::InProgress,
        Status::Win(m) => StatusView::Win { winner: mark_char(m) },
        Status::Draw => StatusView::Draw,
    };
    GameView {
        cells,
        turn: mark_char(game.board.turn_mark()),
        human_mark: mark_char(game.human),
        awaiting_ai: game.status() == Status::InProgress && !game.is_human_turn(),
        status,
    }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// Start a new game. `mark` is the human's mark; `human_starts` decides who opens.
#[tauri::command]
fn new_game(mark: char, human_starts: bool, state: State<'_, AppState>) -> Result<GameView, String> {
    let human = parse_mark(mark)?;
    let game = Game::new(human, human_starts);
    let v = view(&game);
    *state.game.lock().unwrap() = Some(game);
    Ok(v)
}

/// Apply the human's move at `cell`. Rejected if it is not the human's turn or
/// the move is illegal.
#[tauri::command]
fn play(cell: usize, state: State<'_, AppState>) -> Result<GameView, String> {
    let mut guard = state.game.lock().unwrap();
    let game = guard.as_mut().ok_or("no game in progress")?;
    if !game.is_human_turn() {
        return Err("it is not your turn".into());
    }
    game.play(cell).map_err(|e| e.to_string())?;
    Ok(view(game))
}

/// Compute and apply the computer's move. The search runs on a blocking worker
/// thread so the UI never freezes; a `thinking` event brackets the work.
#[tauri::command]
async fn ai_move(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<GameView, String> {
    // Snapshot the board without holding the lock across the await.
    let board = {
        let guard = state.game.lock().unwrap();
        let game = guard.as_ref().ok_or("no game in progress")?;
        if game.is_human_turn() {
            return Err("it is not the computer's turn".into());
        }
        if game.status() != Status::InProgress {
            return Err("the game is over".into());
        }
        game.board
    };

    let _ = app.emit(THINKING_EVENT, true);
    let mv = tauri::async_runtime::spawn_blocking(move || ttt_core::best_move(&board))
        .await
        .map_err(|e| format!("search task failed: {e}"))?;
    let _ = app.emit(THINKING_EVENT, false);

    // Re-acquire the lock and apply, re-validating in case the game was reset
    // while the search ran.
    let mut guard = state.game.lock().unwrap();
    let game = guard.as_mut().ok_or("no game in progress")?;
    if game.is_human_turn() || game.status() != Status::InProgress {
        return Ok(view(game));
    }
    if let Some(Move { cell }) = mv {
        game.play(cell).map_err(|e| e.to_string())?;
    }
    Ok(view(game))
}

fn main() {
    tauri::Builder::default()
        .manage(AppState {
            game: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![new_game, play, ai_move])
        .run(tauri::generate_context!())
        .expect("error while running the Tic-Tac-Toe application");
}
