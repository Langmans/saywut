import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import type { Line } from "../session";
import SpeakerLabel from "./SpeakerLabel.vue";
import TranscriptLine from "./TranscriptLine.vue";

const line = (over: Partial<Line>): Line => ({
  key: 0,
  source: 0,
  text: "Good morning.",
  partial: false,
  ...over,
});

const speakers = { "0": "David" };

describe("TranscriptLine", () => {
  it("labels a known speaker", () => {
    const w = mount(TranscriptLine, { props: { line: line({ speaker: 0 }), speakers } });
    expect(w.findComponent(SpeakerLabel).props()).toEqual({ id: 0, label: "David" });
    expect(w.find(".text").text()).toBe("Good morning.");
    expect(w.classes()).not.toContain("partial");
  });

  it("falls back to the id for an unknown speaker", () => {
    const w = mount(TranscriptLine, { props: { line: line({ speaker: 4 }), speakers } });
    expect(w.findComponent(SpeakerLabel).props("label")).toBe("#4");
  });

  it("shows no label without a speaker and marks partial lines", () => {
    const w = mount(TranscriptLine, { props: { line: line({ partial: true }), speakers } });
    expect(w.findComponent(SpeakerLabel).exists()).toBe(false);
    expect(w.classes()).toContain("partial");
  });
});
