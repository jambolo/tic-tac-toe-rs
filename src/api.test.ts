// Verifies the api module's side of the wire protocol: that each helper invokes
// the right Tauri command with the right argument shape, and passes the backend's
// response through untouched. Uses Tauri's official IPC mocks, so no backend is
// needed. The Rust side of the same contract is pinned down in
// `src-tauri/src/main.rs`'s tests.

import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it } from "vitest";
import { api, type GameView } from "./api";

function emptyView(): GameView {
  return {
    cells: Array(9).fill(null),
    turn: "X",
    humanMark: "X",
    status: { kind: "inProgress" },
    awaitingAi: false,
  };
}

/** Record every invoke; answer game commands with `response`. */
function recordIPC(response: GameView) {
  const calls: Array<{ cmd: string; args: unknown }> = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    // Internal event-plugin commands (from `listen`) expect a numeric id.
    return cmd.startsWith("plugin:event|") ? 1 : response;
  });
  return calls;
}

afterEach(() => {
  clearMocks();
});

describe("api", () => {
  it("newGame invokes new_game with who starts", async () => {
    const response = emptyView();
    const calls = recordIPC(response);
    const result = await api.newGame(false);
    expect(calls).toEqual([
      { cmd: "new_game", args: { humanStarts: false } },
    ]);
    expect(result).toEqual(response);
  });

  it("play invokes play with the cell index", async () => {
    const calls = recordIPC(emptyView());
    await api.play(4);
    expect(calls).toEqual([{ cmd: "play", args: { cell: 4 } }]);
  });

  it("aiMove invokes ai_move with no arguments", async () => {
    const calls = recordIPC(emptyView());
    await api.aiMove();
    expect(calls).toHaveLength(1);
    expect(calls[0].cmd).toBe("ai_move");
  });

  it("rejects when the backend rejects", async () => {
    mockIPC(() => {
      throw "that cell is already taken";
    });
    await expect(api.play(0)).rejects.toBe("that cell is already taken");
  });

  it("onThinking subscribes to the ttt://thinking event channel", async () => {
    const calls = recordIPC(emptyView());
    const unlisten = await api.onThinking(() => {});
    expect(typeof unlisten).toBe("function");
    const listenCall = calls.find((c) => c.cmd === "plugin:event|listen");
    expect(listenCall).toBeDefined();
    expect((listenCall!.args as { event: string }).event).toBe("ttt://thinking");
  });
});
