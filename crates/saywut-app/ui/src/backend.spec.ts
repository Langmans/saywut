import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.fn();
const listen = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen }));

const backend = await import("./backend");

beforeEach(() => {
  invoke.mockReset().mockResolvedValue("ok");
  listen.mockReset();
});

describe("backend", () => {
  it("calls the Rust commands by name", async () => {
    await backend.uiStrings();
    await backend.startSession();
    await backend.stopSession();
    await backend.setAlwaysOnTop(true);
    expect(invoke.mock.calls).toEqual([
      ["ui_strings"],
      ["start"],
      ["stop"],
      ["set_always_on_top", { on: true }],
    ]);
  });

  it("hands the payload of each live event to the handler", async () => {
    const unlisten = () => {};
    listen.mockResolvedValue(unlisten);
    const handler = vi.fn();
    expect(await backend.onLiveEvent(handler)).toBe(unlisten);
    const [name, callback] = listen.mock.calls[0];
    expect(name).toBe(backend.LIVE_EVENT);
    callback({ payload: { kind: "stopped", at_ms: 5 } });
    expect(handler).toHaveBeenCalledWith({ kind: "stopped", at_ms: 5 });
  });
});
