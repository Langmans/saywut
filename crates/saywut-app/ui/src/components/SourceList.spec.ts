import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import LevelMeter from "./LevelMeter.vue";
import SourceChip from "./SourceChip.vue";
import SourceList from "./SourceList.vue";

describe("SourceList", () => {
  it("shows a chip and a meter per source", () => {
    const w = mount(SourceList, {
      props: {
        sources: [
          { source: { id: 0, kind: "microphone", name: "Mic" }, dbfs: -20 },
          { source: { id: 1, kind: "file", name: "clip.wav" }, dbfs: -40 },
        ],
      },
    });
    expect(w.findAllComponents(SourceChip).map((c) => c.props("source").name)).toEqual([
      "Mic",
      "clip.wav",
    ]);
    expect(w.findAllComponents(LevelMeter).map((m) => m.props("dbfs"))).toEqual([-20, -40]);
  });

  it("renders nothing without sources", () => {
    expect(mount(SourceList, { props: { sources: [] } }).find(".sources").exists()).toBe(false);
  });
});
