import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import { setStrings } from "../i18n";
import SessionControls from "./SessionControls.vue";

setStrings({
  lang: "en",
  messages: {
    "app-start": "Start",
    "app-stop": "Stop",
    "app-status-live": "Live",
    "app-status-stopped": "Stopped",
  },
});

describe("SessionControls", () => {
  it("offers Start while stopped", async () => {
    const w = mount(SessionControls, { props: { running: false } });
    expect(w.find(".status").text()).toBe("Stopped");
    expect(w.find(".status").classes()).not.toContain("live");
    await w.find("button").trigger("click");
    expect(w.find("button").text()).toBe("Start");
    expect(w.emitted()).toHaveProperty("start");
    expect(w.emitted()).not.toHaveProperty("stop");
  });

  it("offers Stop while live", async () => {
    const w = mount(SessionControls, { props: { running: true } });
    expect(w.find(".status").text()).toBe("Live");
    expect(w.find(".status").classes()).toContain("live");
    await w.find("button").trigger("click");
    expect(w.find("button").text()).toBe("Stop");
    expect(w.emitted()).toHaveProperty("stop");
    expect(w.emitted()).not.toHaveProperty("start");
  });
});
