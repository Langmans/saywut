import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LiveEvent } from "./live";

const backend = vi.hoisted(() => ({
  uiStrings: vi.fn(),
  startSession: vi.fn(),
  stopSession: vi.fn(),
  setAlwaysOnTop: vi.fn(),
  onLiveEvent: vi.fn(),
}));
vi.mock("./backend", () => backend);

const { default: App } = await import("./App.vue");

let emit: (event: LiveEvent) => void;

beforeEach(() => {
  for (const fn of Object.values(backend)) fn.mockReset();
  backend.uiStrings.mockResolvedValue({
    lang: "nl",
    messages: { "app-title": "Saywut", "app-start": "Start", "app-stop": "Stop" },
  });
  backend.startSession.mockResolvedValue({ speakers: { "0": "David" } });
  backend.stopSession.mockResolvedValue(undefined);
  backend.onLiveEvent.mockImplementation(async (handler) => {
    emit = handler;
    return () => {};
  });
});

async function mounted() {
  const w = mount(App);
  await flushPromises();
  return w;
}

describe("App", () => {
  it("loads strings, listens, then starts a session", async () => {
    const w = await mounted();
    expect(w.find("h1").text()).toBe("Saywut");
    expect(document.documentElement.lang).toBe("nl");
    const listened = backend.onLiveEvent.mock.invocationCallOrder[0];
    expect(listened).toBeLessThan(backend.startSession.mock.invocationCallOrder[0]);
  });

  it("renders live events with speaker labels", async () => {
    const w = await mounted();
    emit({ kind: "started", sources: [{ id: 0, kind: "file", name: "clip.wav" }] });
    emit({ kind: "partial", source: 0, line: { start_ms: 0, speaker: 0, text: "Good" } });
    await flushPromises();
    expect(w.find(".chip").text()).toContain("clip.wav");
    expect(w.find(".line.partial").text()).toBe("DavidGood");
    emit({
      kind: "final",
      source: 0,
      segment: { start_ms: 0, end_ms: 9, speaker: 0, text: "Good morning." },
    });
    await flushPromises();
    expect(w.find(".line").classes()).not.toContain("partial");
    expect(w.find("button").text()).toBe("Stop");
  });

  it("stops and restarts from the button", async () => {
    const w = await mounted();
    emit({ kind: "started", sources: [] });
    await flushPromises();
    await w.find("button").trigger("click");
    expect(backend.stopSession).toHaveBeenCalledOnce();
    emit({ kind: "stopped", at_ms: 0 });
    await flushPromises();
    await w.find("button").trigger("click");
    expect(backend.startSession).toHaveBeenCalledTimes(2);
  });

  it("shows why a session could not start", async () => {
    backend.startSession.mockRejectedValue("Kon audiobestand x.wav niet lezen");
    const w = await mounted();
    expect(w.find("[role=alert]").text()).toBe("Kon audiobestand x.wav niet lezen");
  });

  it("shows an always-on-top failure", async () => {
    backend.setAlwaysOnTop.mockRejectedValue("refused");
    const w = await mounted();
    await w.find("input[type=checkbox]").setValue(true);
    await flushPromises();
    expect(w.find("[role=alert]").text()).toBe("refused");
  });
});
