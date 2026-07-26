// The single door between the UI and the Rust backend.
//
// Every backend interaction goes through this module: nothing else in the front
// end imports from `@tauri-apps/api` directly. That keeps the Rust<->JS boundary
// in one place, so if the protocol changes there is exactly one file to update.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Mark = "X" | "O";
export type Cell = Mark | null;

export type Status =
  | { kind: "inProgress" }
  | { kind: "win"; winner: Mark }
  | { kind: "draw" };

/** Snapshot of the authoritative game, mirrored from the Rust `GameView`. */
export interface GameView {
  cells: Cell[];
  turn: Mark;
  humanMark: Mark;
  status: Status;
  awaitingAi: boolean;
}

export const api = {
  /** Start a new game. X always opens; `humanStarts` decides whether the human plays X (first) or O (second). */
  newGame(humanStarts: boolean): Promise<GameView> {
    return invoke<GameView>("new_game", { humanStarts });
  },

  /** Apply the human's move at `cell` (0..8). */
  play(cell: number): Promise<GameView> {
    return invoke<GameView>("play", { cell });
  },

  /** Ask the computer to compute and play its move. */
  aiMove(): Promise<GameView> {
    return invoke<GameView>("ai_move");
  },

  /** Subscribe to the backend's "computer is thinking" signal. */
  onThinking(cb: (thinking: boolean) => void): Promise<UnlistenFn> {
    return listen<boolean>("ttt://thinking", (event) => cb(event.payload));
  },
};
