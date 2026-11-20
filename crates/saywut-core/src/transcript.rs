//! Transcript model: the source of truth that every export is derived from.
//!
//! Times are whole milliseconds from the start of the recording. Milliseconds
//! are what SRT and VTT can express, so nothing is lost on export and the JSON
//! stays free of float rounding.

use serde::{Deserialize, Serialize};

/// Version of the JSON layout written by [`Transcript::to_json`]. Bumped when a
/// change would make an older reader misread the file.
pub const FORMAT_VERSION: u32 = 1;

/// Stable identifier of a speaker within one transcript.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SpeakerId(pub u32);

/// A speaker found by diarization. `name` stays `None` until the user names
/// them; exports then fall back to a localized label such as "Speaker 1".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Speaker {
    pub id: SpeakerId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// One stretch of speech by at most one speaker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub start_ms: u64,
    pub end_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<SpeakerId>,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transcript {
    pub version: u32,
    /// BCP-47 tag of the spoken language, `None` when it was not detected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default)]
    pub speakers: Vec<Speaker>,
    #[serde(default)]
    pub segments: Vec<Segment>,
}

#[derive(Debug)]
pub enum TranscriptError {
    Json(serde_json::Error),
    UnsupportedVersion(u32),
}

impl std::fmt::Display for TranscriptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "invalid transcript JSON: {e}"),
            Self::UnsupportedVersion(v) => write!(
                f,
                "transcript format version {v} is newer than supported version {FORMAT_VERSION}"
            ),
        }
    }
}

impl std::error::Error for TranscriptError {}

impl Default for Transcript {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            language: None,
            speakers: Vec::new(),
            segments: Vec::new(),
        }
    }
}

impl Transcript {
    pub fn speaker(&self, id: SpeakerId) -> Option<&Speaker> {
        self.speakers.iter().find(|s| s.id == id)
    }

    /// Pretty-printed JSON, so the file diffs well and can be read by hand.
    pub fn to_json(&self) -> String {
        // Serializing plain structs with string keys cannot fail.
        serde_json::to_string_pretty(self).expect("transcript serializes")
    }

    pub fn from_json(json: &str) -> Result<Self, TranscriptError> {
        let transcript: Self = serde_json::from_str(json).map_err(TranscriptError::Json)?;
        if transcript.version > FORMAT_VERSION {
            return Err(TranscriptError::UnsupportedVersion(transcript.version));
        }
        Ok(transcript)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Transcript {
        Transcript {
            language: Some("nl".into()),
            speakers: vec![
                Speaker {
                    id: SpeakerId(0),
                    name: Some("Ruben".into()),
                },
                Speaker {
                    id: SpeakerId(1),
                    name: None,
                },
            ],
            segments: vec![
                Segment {
                    start_ms: 0,
                    end_ms: 1500,
                    speaker: Some(SpeakerId(0)),
                    text: "Hallo.".into(),
                },
                Segment {
                    start_ms: 1600,
                    end_ms: 3000,
                    speaker: None,
                    text: "Hoi.".into(),
                },
            ],
            ..Transcript::default()
        }
    }

    #[test]
    fn default_has_current_version() {
        assert_eq!(Transcript::default().version, FORMAT_VERSION);
    }

    #[test]
    fn json_round_trips() {
        let t = sample();
        assert_eq!(Transcript::from_json(&t.to_json()).unwrap(), t);
    }

    #[test]
    fn json_omits_absent_optionals() {
        let json = sample().to_json();
        // Speaker 1 has no name and segment 2 has no speaker.
        assert_eq!(json.matches("\"name\"").count(), 1);
        assert_eq!(json.matches("\"speaker\"").count(), 1);
        assert!(json.contains("\"speaker\": 0"));
    }

    #[test]
    fn minimal_json_reads_with_defaults() {
        let t = Transcript::from_json(r#"{"version":1}"#).unwrap();
        assert_eq!(t, Transcript::default());
    }

    #[test]
    fn newer_version_is_rejected() {
        let err = Transcript::from_json(r#"{"version":2}"#).unwrap_err();
        assert!(matches!(err, TranscriptError::UnsupportedVersion(2)));
        assert!(err.to_string().contains("version 2"));
    }

    #[test]
    fn malformed_json_is_rejected() {
        let err = Transcript::from_json("{").unwrap_err();
        assert!(matches!(err, TranscriptError::Json(_)));
        assert!(err.to_string().starts_with("invalid transcript JSON"));
    }

    #[test]
    fn speaker_lookup() {
        let t = sample();
        assert_eq!(
            t.speaker(SpeakerId(0)).unwrap().name.as_deref(),
            Some("Ruben")
        );
        assert!(t.speaker(SpeakerId(7)).is_none());
    }
}
