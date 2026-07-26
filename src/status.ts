// Pure presentation logic, split out of main.ts specifically so it can be unit
// tested without a DOM: given a game snapshot (and the thinking flag), decide
// what the status line should say. No side effects, no globals.

import type { GameView } from "./api";

export function statusText(view: GameView, thinking: boolean): string {
  const human = view.humanMark;
  switch (view.status.kind) {
    case "win":
      return view.status.winner === human ? "You win! 🎉" : "Computer wins.";
    case "draw":
      return "It's a draw.";
    case "inProgress":
      if (view.awaitingAi) return thinking ? "Computer is thinking…" : "Computer's turn…";
      return `Your turn (${human}).`;
  }
}
