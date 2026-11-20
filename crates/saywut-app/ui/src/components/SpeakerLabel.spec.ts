import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SpeakerLabel from "./SpeakerLabel.vue";

describe("SpeakerLabel", () => {
  it("shows the label in the speaker's colour", () => {
    const w = mount(SpeakerLabel, { props: { id: 1, label: "Zira" } });
    expect(w.text()).toBe("Zira");
    expect(w.classes()).toContain("s1");
  });

  it("reuses the palette beyond six speakers", () => {
    expect(mount(SpeakerLabel, { props: { id: 7, label: "Speaker 8" } }).classes()).toContain("s1");
  });
});
