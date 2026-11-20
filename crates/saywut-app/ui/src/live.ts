// The shapes the Rust side sends; they mirror saywut_core::live and the
// commands in crates/saywut-app/src/lib.rs.

export type SourceKind = "microphone" | "loopback" | "application" | "file";

export interface Source {
  id: number;
  kind: SourceKind;
  name: string;
}

export interface PartialLine {
  start_ms: number;
  speaker?: number;
  text: string;
}

export interface Segment {
  start_ms: number;
  end_ms: number;
  speaker?: number;
  text: string;
}

export type LiveEvent =
  | { kind: "started"; sources: Source[] }
  | { kind: "level"; source: number; at_ms: number; dbfs: number }
  | { kind: "partial"; source: number; line: PartialLine }
  | { kind: "final"; source: number; segment: Segment }
  | { kind: "stopped"; at_ms: number };

export interface UiStrings {
  lang: string;
  messages: Record<string, string>;
}

export interface SessionInfo {
  /** Display label per speaker id (JSON object keys are strings). */
  speakers: Record<string, string>;
}
