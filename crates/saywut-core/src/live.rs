//! Events of a live session: what a running session reports to the UI,
//! from [`LiveEvent::Started`] to [`LiveEvent::Stopped`], with at most one
//! partial line per source at a time. Serialized as JSON tagged by `"kind"`;
//! see the crate README.

use serde::{Deserialize, Serialize};

use crate::level::rms_dbfs;
use crate::transcript::{Segment, SpeakerId};

/// Identifies one source within a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceId(pub u32);

/// What a source captures. The UI picks an icon and a localized name from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    /// A microphone or other input device.
    Microphone,
    /// Everything an output device plays.
    Loopback,
    /// The audio of one application.
    Application,
    /// An audio file played as if it were live.
    File,
}

/// One audio source of a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    /// Its id within the session.
    pub id: SourceId,
    /// What it captures.
    pub kind: SourceKind,
    /// Device, application or file name as the system reports it; not
    /// localized.
    pub name: String,
}

/// Text recognized so far for a stretch of speech that is not final yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartialLine {
    /// Start, in ms since the session started.
    pub start_ms: u64,
    /// Who speaks, if already known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<SpeakerId>,
    /// The text so far.
    pub text: String,
}

/// What a running session reports. Times are milliseconds since the session
/// started.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LiveEvent {
    /// The session runs; always the first event.
    Started {
        /// The sources it records.
        sources: Vec<Source>,
    },
    /// RMS level of the audio a source captured up to `at_ms`.
    Level {
        /// The source measured.
        source: SourceId,
        /// End of the measured audio.
        at_ms: u64,
        /// Level in dBFS, never below [`MIN_DBFS`](crate::level::MIN_DBFS).
        dbfs: f32,
    },
    /// The current not-yet-final line of a source, replacing the previous one.
    Partial {
        /// The source speaking.
        source: SourceId,
        /// The line so far.
        line: PartialLine,
    },
    /// A finished segment, replacing the source's partial line.
    Final {
        /// The source speaking.
        source: SourceId,
        /// The finished segment.
        segment: Segment,
    },
    /// The session ended; always the last event.
    Stopped {
        /// When it ended.
        at_ms: u64,
    },
}

impl LiveEvent {
    /// A level event measured over `samples` (range -1.0..=1.0).
    pub fn level(source: SourceId, at_ms: u64, samples: &[f32]) -> Self {
        Self::Level {
            source,
            at_ms,
            dbfs: rms_dbfs(samples),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::level::MIN_DBFS;
    use serde_json::{Value, json};

    fn to_value(event: &LiveEvent) -> Value {
        serde_json::to_value(event).unwrap()
    }

    fn round_trips(event: LiveEvent) {
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(serde_json::from_str::<LiveEvent>(&json).unwrap(), event);
    }

    #[test]
    fn started_lists_sources() {
        let event = LiveEvent::Started {
            sources: vec![Source {
                id: SourceId(0),
                kind: SourceKind::File,
                name: "en-dialogue.wav".into(),
            }],
        };
        assert_eq!(
            to_value(&event),
            json!({
                "kind": "started",
                "sources": [{"id": 0, "kind": "file", "name": "en-dialogue.wav"}]
            })
        );
        round_trips(event);
    }

    #[test]
    fn source_kinds_are_snake_case() {
        let kinds = [
            (SourceKind::Microphone, "microphone"),
            (SourceKind::Loopback, "loopback"),
            (SourceKind::Application, "application"),
            (SourceKind::File, "file"),
        ];
        for (kind, name) in kinds {
            assert_eq!(serde_json::to_value(kind).unwrap(), json!(name));
        }
    }

    #[test]
    fn level_is_measured_in_dbfs() {
        let event = LiveEvent::level(SourceId(1), 50, &[0.5, -0.5]);
        let dbfs = to_value(&event)["dbfs"].as_f64().unwrap();
        assert!((dbfs + 6.02).abs() < 0.01, "{dbfs}");
        assert_eq!(
            to_value(&LiveEvent::level(SourceId(1), 50, &[])),
            json!({"kind": "level", "source": 1, "at_ms": 50, "dbfs": MIN_DBFS})
        );
        round_trips(event);
    }

    #[test]
    fn partial_omits_unknown_speaker() {
        let event = LiveEvent::Partial {
            source: SourceId(0),
            line: PartialLine {
                start_ms: 1200,
                speaker: None,
                text: "Good morning".into(),
            },
        };
        assert_eq!(
            to_value(&event),
            json!({
                "kind": "partial",
                "source": 0,
                "line": {"start_ms": 1200, "text": "Good morning"}
            })
        );
        round_trips(event);
    }

    #[test]
    fn final_carries_the_segment() {
        let event = LiveEvent::Final {
            source: SourceId(0),
            segment: Segment {
                start_ms: 0,
                end_ms: 900,
                speaker: Some(SpeakerId(1)),
                text: "Hallo.".into(),
            },
        };
        assert_eq!(
            to_value(&event),
            json!({
                "kind": "final",
                "source": 0,
                "segment": {"start_ms": 0, "end_ms": 900, "speaker": 1, "text": "Hallo."}
            })
        );
        round_trips(event);
    }

    #[test]
    fn stopped_reports_its_time() {
        let event = LiveEvent::Stopped { at_ms: 30_000 };
        assert_eq!(
            to_value(&event),
            json!({"kind": "stopped", "at_ms": 30_000})
        );
        round_trips(event);
    }

    #[test]
    fn unknown_kind_is_an_error() {
        assert!(serde_json::from_str::<LiveEvent>(r#"{"kind":"nope"}"#).is_err());
    }
}
