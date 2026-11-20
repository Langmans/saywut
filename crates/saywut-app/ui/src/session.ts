// What the window shows of a session, built up from LiveEvents.
//
// Each source has at most one partial line; it sits in the transcript where
// it first appeared, is rewritten in place as words arrive, and turns into
// the final line when the segment is done.

import type { LiveEvent, Source } from "./live";

/** Level shown for silence; matches saywut_core::level::MIN_DBFS. */
export const MIN_DBFS = -120;

export interface SourceState {
  source: Source;
  dbfs: number;
}

export interface Line {
  /** Stable key for list rendering. */
  key: number;
  source: number;
  speaker?: number;
  text: string;
  partial: boolean;
}

export interface SessionState {
  running: boolean;
  sources: SourceState[];
  lines: Line[];
  nextKey: number;
}

export function emptySession(): SessionState {
  return { running: false, sources: [], lines: [], nextKey: 0 };
}

function partialOf(state: SessionState, source: number): Line | undefined {
  // Partial lines are near the end, so search from there.
  return state.lines.findLast((l) => l.partial && l.source === source);
}

function addLine(state: SessionState, line: Omit<Line, "key">): void {
  state.lines.push({ ...line, key: state.nextKey++ });
}

export function applyEvent(state: SessionState, event: LiveEvent): void {
  switch (event.kind) {
    case "started":
      state.running = true;
      state.sources = event.sources.map((source) => ({ source, dbfs: MIN_DBFS }));
      state.lines = [];
      break;
    case "level": {
      const s = state.sources.find((s) => s.source.id === event.source);
      if (s) s.dbfs = event.dbfs;
      break;
    }
    case "partial": {
      const { speaker, text } = event.line;
      const line = partialOf(state, event.source);
      if (line) Object.assign(line, { speaker, text });
      else addLine(state, { source: event.source, speaker, text, partial: true });
      break;
    }
    case "final": {
      const { speaker, text } = event.segment;
      const line = partialOf(state, event.source);
      if (line) Object.assign(line, { speaker, text, partial: false });
      else addLine(state, { source: event.source, speaker, text, partial: false });
      break;
    }
    case "stopped":
      state.running = false;
      state.lines = state.lines.filter((l) => !l.partial);
      for (const s of state.sources) s.dbfs = MIN_DBFS;
      break;
  }
}
