import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import { setStrings } from "../i18n";
import type { SourceKind } from "../live";
import SourceChip from "./SourceChip.vue";

describe("SourceChip", () => {
  setStrings({
    lang: "en",
    messages: {
      "app-source-microphone": "Microphone",
      "app-source-loopback": "System audio",
      "app-source-application": "App",
      "app-source-file": "File",
    },
  });

  it.each([
    ["microphone", "Microphone"],
    ["loopback", "System audio"],
    ["application", "App"],
    ["file", "File"],
  ] as [SourceKind, string][])("names a %s source", (kind, label) => {
    const w = mount(SourceChip, { props: { source: { id: 0, kind, name: "x.wav" } } });
    expect(w.text()).toBe(`${label} · x.wav`);
    expect(w.classes()).toContain(kind);
    expect(w.attributes("title")).toBe("x.wav");
  });
});
