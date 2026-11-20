import { flushPromises, mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import { setStrings } from "../i18n";
import type { Line } from "../session";
import TranscriptLine from "./TranscriptLine.vue";
import TranscriptView from "./TranscriptView.vue";

const line = (key: number, text: string): Line => ({ key, source: 0, text, partial: false });

/** Gives the scroll box a fixed geometry, since happy-dom has no layout. */
function geometry(el: HTMLElement, scrollHeight: number, clientHeight: number) {
  Object.defineProperty(el, "scrollHeight", { configurable: true, get: () => scrollHeight });
  Object.defineProperty(el, "clientHeight", { configurable: true, get: () => clientHeight });
}

describe("TranscriptView", () => {
  it("shows a placeholder while empty, then the lines", async () => {
    setStrings({ lang: "en", messages: { "app-transcript-empty": "Nothing said yet." } });
    const w = mount(TranscriptView, { props: { lines: [], speakers: {} } });
    expect(w.find(".empty").text()).toBe("Nothing said yet.");
    await w.setProps({ lines: [line(0, "a"), line(1, "b")] });
    expect(w.find(".empty").exists()).toBe(false);
    expect(w.findAllComponents(TranscriptLine)).toHaveLength(2);
  });

  it("follows new lines while at the bottom", async () => {
    const w = mount(TranscriptView, { props: { lines: [], speakers: {} } });
    const el = w.element as HTMLElement;
    geometry(el, 500, 200);
    el.scrollTop = 290;
    await w.setProps({ lines: [line(0, "a")] });
    await flushPromises();
    expect(el.scrollTop).toBe(500);
  });

  it("stays put when the reader scrolled up", async () => {
    const w = mount(TranscriptView, { props: { lines: [], speakers: {} } });
    const el = w.element as HTMLElement;
    geometry(el, 500, 200);
    el.scrollTop = 100;
    await w.setProps({ lines: [line(0, "a")] });
    await flushPromises();
    expect(el.scrollTop).toBe(100);
  });
});
