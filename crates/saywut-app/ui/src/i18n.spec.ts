import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { setStrings, t } from "./i18n";

describe("t", () => {
  it("shows the key until strings are loaded, then the message", () => {
    expect(t("app-start")).toBe("app-start");
    setStrings({ lang: "nl", messages: { "app-start": "Start" } });
    expect(t("app-start")).toBe("Start");
    expect(t("app-missing")).toBe("app-missing");
    expect(document.documentElement.lang).toBe("nl");
  });
});

/** Every `t("…")` key in the frontend sources. */
function usedKeys(): Set<string> {
  const dir = import.meta.dirname;
  const keys = new Set<string>();
  const files = readdirSync(dir, { recursive: true, encoding: "utf8" }).filter(
    (f) => /\.(ts|vue)$/.test(f) && !f.endsWith(".spec.ts"),
  );
  for (const file of files) {
    const source = readFileSync(join(dir, file), "utf8");
    for (const m of source.matchAll(/(?<![\w.])t\(["']([^"']+)["']\)/g)) keys.add(m[1]);
  }
  return keys;
}

describe("frontend keys", () => {
  // Rust sends only app-* messages, and every locale has the same keys
  // (checked in saywut-i18n), so English is the reference.
  const ftl = readFileSync(join(import.meta.dirname, "../../../../locales/en/saywut.ftl"), "utf8");
  const defined = new Set([...ftl.matchAll(/^([\w-]+) =/gm)].map((m) => m[1]));

  it("finds the keys in templates and scripts", () => {
    const keys = usedKeys();
    expect(keys).toContain("app-title");
    expect(keys).toContain("app-source-file");
  });

  it.each([...usedKeys()])("%s is an app-* message in the locales", (key) => {
    expect(key.startsWith("app-")).toBe(true);
    expect(defined).toContain(key);
  });
});
