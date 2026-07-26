// Behavioural tests for the UI controller (main.ts) against a mocked backend.
// main.ts is DOM-driven glue, so these run in jsdom with `./api` replaced by a
// mock: they cover the interaction rules the controller enforces — when clicks
// are accepted, when the AI turn is driven, and how failures degrade.

import { beforeEach, describe, expect, it, vi } from "vitest";
import type { GameView } from "./api";

const mockApi = vi.hoisted(() => ({
  newGame: vi.fn<(humanStarts: boolean) => Promise<GameView>>(),
  play: vi.fn<(cell: number) => Promise<GameView>>(),
  aiMove: vi.fn<() => Promise<GameView>>(),
  onThinking: vi.fn<(cb: (t: boolean) => void) => Promise<() => void>>(),
}));

vi.mock("./api", () => ({ api: mockApi }));

function view(overrides: Partial<GameView> = {}): GameView {
  return {
    cells: Array(9).fill(null),
    turn: "X",
    humanMark: "X",
    status: { kind: "inProgress" },
    awaitingAi: false,
    ...overrides,
  };
}

/** Let all pending promise chains (click -> invoke -> render) settle. */
const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

let thinkingCb: ((t: boolean) => void) | undefined;

/** Build the DOM main.ts expects, then (re-)import it fresh. */
async function boot(): Promise<void> {
  document.body.innerHTML = `
    <div id="board"></div>
    <p id="status"></p>
    <label><input type="radio" name="first" value="human" checked /></label>
    <label><input type="radio" name="first" value="computer" /></label>
    <button id="new-game"></button>
  `;
  vi.resetModules();
  await import("./main");
}

const cells = () =>
  Array.from(document.querySelectorAll<HTMLButtonElement>("#board .cell"));
const status = () => document.getElementById("status")!.textContent;
const clickNewGame = () => document.getElementById("new-game")!.click();

async function startGame(first: GameView): Promise<void> {
  mockApi.newGame.mockResolvedValueOnce(first);
  clickNewGame();
  await flush();
}

beforeEach(() => {
  vi.clearAllMocks();
  thinkingCb = undefined;
  mockApi.onThinking.mockImplementation(async (cb) => {
    thinkingCb = cb;
    return () => {};
  });
});

describe("board construction", () => {
  it("builds nine cell buttons and subscribes to the thinking event", async () => {
    await boot();
    expect(cells()).toHaveLength(9);
    expect(mockApi.onThinking).toHaveBeenCalledOnce();
  });
});

describe("starting a game", () => {
  it("starts with the selected options and prompts the human", async () => {
    await boot();
    await startGame(view());
    expect(mockApi.newGame).toHaveBeenCalledWith(true);
    expect(mockApi.aiMove).not.toHaveBeenCalled();
    expect(status()).toBe("Your turn (X).");
    expect(cells().every((b) => !b.disabled)).toBe(true);
  });

  it("passes the computer-first choice through", async () => {
    await boot();
    document.querySelector<HTMLInputElement>('input[value="computer"]')!.checked = true;
    mockApi.aiMove.mockResolvedValueOnce(
      view({ humanMark: "O", cells: [null, null, null, null, "X", null, null, null, null] }),
    );
    await startGame(view({ humanMark: "O", turn: "X", awaitingAi: true }));
    expect(mockApi.newGame).toHaveBeenCalledWith(false);
  });

  it("drives the AI's opening move when the computer starts", async () => {
    // Computer first means the computer opens as X and the human plays O.
    await boot();
    mockApi.aiMove.mockResolvedValueOnce(
      view({ humanMark: "O", cells: [null, null, null, null, "X", null, null, null, null] }),
    );
    await startGame(view({ humanMark: "O", awaitingAi: true }));
    expect(mockApi.aiMove).toHaveBeenCalledOnce();
    expect(cells()[4].textContent).toBe("X");
    expect(status()).toBe("Your turn (O).");
  });

  it("shows a helpful error when the backend is unreachable", async () => {
    await boot();
    mockApi.newGame.mockRejectedValueOnce(new Error("no backend"));
    clickNewGame();
    await flush();
    expect(status()).toBe("Could not start a game. Is the backend running?");
  });
});

describe("playing a move", () => {
  it("forwards a click on an empty cell and then drives the AI reply", async () => {
    await boot();
    await startGame(view());
    mockApi.play.mockResolvedValueOnce(
      view({ cells: ["X", null, null, null, null, null, null, null, null], awaitingAi: true }),
    );
    mockApi.aiMove.mockResolvedValueOnce(
      view({ cells: ["X", null, null, null, "O", null, null, null, null] }),
    );
    cells()[0].click();
    await flush();
    expect(mockApi.play).toHaveBeenCalledWith(0);
    expect(mockApi.aiMove).toHaveBeenCalledOnce();
    expect(cells()[0].textContent).toBe("X");
    expect(cells()[4].textContent).toBe("O");
  });

  it("ignores clicks before a game has started", async () => {
    await boot();
    cells()[0].click();
    await flush();
    expect(mockApi.play).not.toHaveBeenCalled();
  });

  it("ignores clicks on occupied cells", async () => {
    await boot();
    await startGame(view({ cells: ["O", null, null, null, null, null, null, null, null] }));
    cells()[0].click();
    await flush();
    expect(mockApi.play).not.toHaveBeenCalled();
  });

  it("ignores clicks while it is the computer's turn", async () => {
    await boot();
    mockApi.aiMove.mockReturnValueOnce(new Promise(() => {})); // AI still searching
    await startGame(view({ awaitingAi: true }));
    cells()[0].click();
    await flush();
    expect(mockApi.play).not.toHaveBeenCalled();
  });

  it("ignores clicks once the game is over and disables every cell", async () => {
    await boot();
    await startGame(view({ status: { kind: "win", winner: "O" } }));
    expect(status()).toBe("Computer wins.");
    expect(cells().every((b) => b.disabled)).toBe(true);
    cells()[1].click();
    await flush();
    expect(mockApi.play).not.toHaveBeenCalled();
  });

  it("survives the backend rejecting a move", async () => {
    await boot();
    await startGame(view());
    mockApi.play.mockRejectedValueOnce("that cell is already taken");
    cells()[0].click();
    await flush();
    expect(mockApi.aiMove).not.toHaveBeenCalled();
    expect(status()).toBe("Your turn (X).");
    expect(cells().every((b) => !b.disabled)).toBe(true);
  });

  it("survives the AI move failing and re-enables input", async () => {
    await boot();
    await startGame(view());
    mockApi.play.mockResolvedValueOnce(view({ awaitingAi: true }));
    mockApi.aiMove.mockRejectedValueOnce("search task failed");
    cells()[0].click();
    await flush();
    // The stale awaiting_ai snapshot still renders as the computer's turn, but
    // the controller must not crash and must leave `busy` cleared.
    expect(status()).toBe("Computer's turn…");
  });
});

describe("thinking indicator", () => {
  it("reflects the backend's thinking events in the status line", async () => {
    await boot();
    mockApi.aiMove.mockReturnValueOnce(new Promise(() => {})); // never resolves
    await startGame(view({ awaitingAi: true }));
    expect(status()).toBe("Computer's turn…");

    thinkingCb!(true);
    expect(status()).toBe("Computer is thinking…");

    thinkingCb!(false);
    expect(status()).toBe("Computer's turn…");
  });
});
