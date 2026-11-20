import { describe, expect, it } from "vitest";
import type { LiveEvent, Source } from "./live";
import { MIN_DBFS, applyEvent, emptySession, type SessionState } from "./session";

const mic: Source = { id: 0, kind: "microphone", name: "Mic" };
const file: Source = { id: 1, kind: "file", name: "clip.wav" };

function run(...events: LiveEvent[]): SessionState {
  const state = emptySession();
  for (const e of events) applyEvent(state, e);
  return state;
}

const started: LiveEvent = { kind: "started", sources: [mic, file] };
const partial = (source: number, text: string, speaker?: number): LiveEvent => ({
  kind: "partial",
  source,
  line: { start_ms: 0, speaker, text },
});
const final = (source: number, text: string, speaker?: number): LiveEvent => ({
  kind: "final",
  source,
  segment: { start_ms: 0, end_ms: 10, speaker, text },
});

describe("applyEvent", () => {
  it("starts with every source silent and an empty transcript", () => {
    const state = run(final(0, "old"), started);
    expect(state.running).toBe(true);
    expect(state.sources).toEqual([
      { source: mic, dbfs: MIN_DBFS },
      { source: file, dbfs: MIN_DBFS },
    ]);
    expect(state.lines).toEqual([]);
  });

  it("sets the level of its source and ignores unknown sources", () => {
    const state = run(
      started,
      { kind: "level", source: 1, at_ms: 50, dbfs: -12 },
      { kind: "level", source: 9, at_ms: 50, dbfs: -3 },
    );
    expect(state.sources.map((s) => s.dbfs)).toEqual([MIN_DBFS, -12]);
  });

  it("grows one partial line per source in place", () => {
    const state = run(started, partial(0, "Good"), partial(1, "Hallo"), partial(0, "Good morning", 2));
    expect(state.lines).toEqual([
      { key: 0, source: 0, speaker: 2, text: "Good morning", partial: true },
      { key: 1, source: 1, speaker: undefined, text: "Hallo", partial: true },
    ]);
  });

  it("turns the partial line into the final one where it stands", () => {
    const state = run(started, partial(0, "Good"), partial(1, "Hal"), final(0, "Good morning.", 0));
    expect(state.lines[0]).toEqual({
      key: 0,
      source: 0,
      speaker: 0,
      text: "Good morning.",
      partial: false,
    });
    // The next partial of that source starts a new line.
    applyEvent(state, partial(0, "Is"));
    expect(state.lines.map((l) => l.key)).toEqual([0, 1, 2]);
  });

  it("adds a final line that had no partial", () => {
    const state = run(started, final(1, "Hoi."));
    expect(state.lines).toEqual([
      { key: 0, source: 1, speaker: undefined, text: "Hoi.", partial: false },
    ]);
  });

  it("stops: drops unfinished lines and silences the meters", () => {
    const state = run(
      started,
      { kind: "level", source: 0, at_ms: 50, dbfs: -6 },
      final(0, "Done."),
      partial(1, "unfinished"),
      { kind: "stopped", at_ms: 100 },
    );
    expect(state.running).toBe(false);
    expect(state.lines.map((l) => l.text)).toEqual(["Done."]);
    expect(state.sources.every((s) => s.dbfs === MIN_DBFS)).toBe(true);
  });
});
