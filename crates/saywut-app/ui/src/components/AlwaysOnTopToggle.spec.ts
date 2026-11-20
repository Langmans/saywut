import { flushPromises, mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it, vi } from "vitest";

// A plain function rather than vi.fn: Vitest's spy tracks a rejected
// promise on its own and reports it as unhandled, although the component
// handles it.
let calls: boolean[] = [];
let answer: () => Promise<void>;
vi.mock("../backend", () => ({
  setAlwaysOnTop: (on: boolean) => {
    calls.push(on);
    return answer();
  },
}));

const { default: AlwaysOnTopToggle } = await import("./AlwaysOnTopToggle.vue");

beforeEach(() => {
  calls = [];
});

describe("AlwaysOnTopToggle", () => {
  it("asks the window to stay on top and back", async () => {
    answer = async () => {};
    const w = mount(AlwaysOnTopToggle);
    const box = w.find("input");
    await box.setValue(true);
    await box.setValue(false);
    await flushPromises();
    expect(calls).toEqual([true, false]);
    expect(w.emitted("error")).toBeUndefined();
  });

  it("flips back and reports when the window refuses", async () => {
    answer = () => Promise.reject("not allowed");
    const w = mount(AlwaysOnTopToggle);
    await w.find("input").setValue(true);
    await flushPromises();
    expect(calls).toEqual([true]);
    expect((w.find("input").element as HTMLInputElement).checked).toBe(false);
    expect(w.emitted("error")).toEqual([["not allowed"]]);
  });
});
