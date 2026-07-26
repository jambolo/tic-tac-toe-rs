import { describe, expect, it } from "vitest";
import type { GameView } from "./api";
import { statusText } from "./status";

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

describe("statusText", () => {
  it("prompts the human when it is their turn", () => {
    expect(statusText(view(), false)).toBe("Your turn (X).");
  });

  it("shows a thinking hint once the AI search has started", () => {
    expect(statusText(view({ awaitingAi: true }), true)).toBe("Computer is thinking…");
  });

  it("shows a generic waiting hint before the AI search has started", () => {
    expect(statusText(view({ awaitingAi: true }), false)).toBe("Computer's turn…");
  });

  it("declares a win when the human's mark matches the winner", () => {
    expect(statusText(view({ status: { kind: "win", winner: "X" } }), false)).toBe("You win! 🎉");
  });

  it("declares a loss when the winner is not the human's mark", () => {
    expect(
      statusText(view({ humanMark: "O", status: { kind: "win", winner: "X" } }), false),
    ).toBe("Computer wins.");
  });

  it("declares a draw", () => {
    expect(statusText(view({ status: { kind: "draw" } }), false)).toBe("It's a draw.");
  });
});
