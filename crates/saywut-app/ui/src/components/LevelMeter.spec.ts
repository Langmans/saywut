import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import LevelMeter from "./LevelMeter.vue";

const meter = (dbfs: number) => mount(LevelMeter, { props: { dbfs } });

describe("LevelMeter", () => {
  it("covers the part above the level", () => {
    const w = meter(-15);
    expect(w.find(".rest").attributes("style")).toContain("width: 25%");
    expect(w.find(".db").text()).toBe("-15 dB");
    expect(w.find("[role=meter]").attributes("aria-valuenow")).toBe("-15");
  });

  it("shows silence below the range as −∞ with an empty bar", () => {
    const w = meter(-120);
    expect(w.find(".rest").attributes("style")).toContain("width: 100%");
    expect(w.find(".db").text()).toBe("−∞");
  });

  it("clamps above full scale to a full bar", () => {
    const w = meter(3);
    expect(w.find(".rest").attributes("style")).toContain("width: 0%");
    expect(w.find(".db").text()).toBe("3 dB");
  });
});
