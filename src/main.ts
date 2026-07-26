// UI controller. Holds no game rules of its own — it renders snapshots returned
// by the backend and forwards user intent through `api`. The Rust core is the
// single source of truth for legality and outcomes.

import { api, type GameView } from "./api";
import { statusText } from "./status";

const boardEl = document.getElementById("board") as HTMLDivElement;
const statusEl = document.getElementById("status") as HTMLParagraphElement;
const newGameBtn = document.getElementById("new-game") as HTMLButtonElement;

let view: GameView | null = null;
/** True while an AI move is in flight; blocks input so the human can't race it. */
let busy = false;
/** Driven by the backend's thinking event, for the status hint. */
let thinking = false;

// Build the nine cell buttons once.
const cellButtons: HTMLButtonElement[] = [];
for (let i = 0; i < 9; i++) {
  const btn = document.createElement("button");
  btn.className = "cell";
  btn.type = "button";
  btn.setAttribute("role", "gridcell");
  btn.dataset.index = String(i);
  btn.addEventListener("click", () => onCellClick(i));
  boardEl.appendChild(btn);
  cellButtons.push(btn);
}

function selectedRadio(name: string): string {
  const el = document.querySelector<HTMLInputElement>(`input[name="${name}"]:checked`);
  return el ? el.value : "";
}

async function startGame(): Promise<void> {
  const humanStarts = selectedRadio("first") !== "computer";
  busy = false;
  thinking = false;
  view = await api.newGame(humanStarts);
  render();
  await maybeAiTurn();
}

async function onCellClick(i: number): Promise<void> {
  if (!view || busy) return;
  if (view.status.kind !== "inProgress") return;
  if (view.awaitingAi) return; // not the human's turn
  if (view.cells[i] !== null) return; // occupied
  try {
    view = await api.play(i);
  } catch (err) {
    // Rust rejected the move (e.g. race); ignore and let the next render correct the UI.
    console.warn("play rejected:", err);
    return;
  }
  render();
  await maybeAiTurn();
}

async function maybeAiTurn(): Promise<void> {
  if (!view) return;
  if (view.status.kind !== "inProgress" || !view.awaitingAi) return;
  busy = true;
  render();
  try {
    view = await api.aiMove();
  } catch (err) {
    console.warn("ai_move failed:", err);
  } finally {
    busy = false;
  }
  render();
}

function render(): void {
  if (!view) return;
  const over = view.status.kind !== "inProgress";
  const humanTurn = !view.awaitingAi && !over;

  view.cells.forEach((mark, i) => {
    const btn = cellButtons[i];
    btn.textContent = mark ?? "";
    btn.classList.toggle("x", mark === "X");
    btn.classList.toggle("o", mark === "O");
    // A cell is playable only when it's the human's turn, the cell is empty,
    // and we're not mid-AI-move.
    const playable = humanTurn && mark === null && !busy;
    btn.disabled = !playable;
    btn.classList.toggle("filled", mark !== null);
  });

  boardEl.classList.toggle("thinking", view.awaitingAi && !over);
  statusEl.textContent = statusText(view, thinking);
}

newGameBtn.addEventListener("click", () => {
  startGame().catch((err) => {
    console.error(err);
    statusEl.textContent = "Could not start a game. Is the backend running?";
  });
});

// Reflect the backend's thinking signal in the status line.
api.onThinking((t) => {
  thinking = t;
  render();
});
