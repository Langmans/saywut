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
    /// Id, counted from 0; labels count from 1.
    pub id: SpeakerId,
    /// Name the user gave, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// One stretch of speech by at most one speaker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    /// Start, in ms from the start of the recording.
    pub start_ms: u64,
    /// End, in ms from the start of the recording.
    pub end_ms: u64,
    /// Who speaks, once diarized.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speaker: Option<SpeakerId>,
    /// What was said.
    pub text: String,
}

/// A whole transcript as stored in its JSON file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transcript {
    /// Layout version, [`FORMAT_VERSION`] when written by this build.
    pub version: u32,
    /// BCP-47 tag of the spoken language, `None` when it was not detected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// Every speaker that occurs, with their name if known.
    #[serde(default)]
    pub speakers: Vec<Speaker>,
    /// The speech, in time order.
    #[serde(default)]
    pub segments: Vec<Segment>,
}

/// Why a transcript JSON could not be read.
#[derive(Debug)]
pub enum TranscriptError {
    /// Not valid transcript JSON.
    Json(serde_json::Error),
    /// Written by a newer Saywut with a layout this build does not know.
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

/// Why a speaker edit was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// No speaker with this id.
    UnknownSpeaker(SpeakerId),
    /// Merging a speaker into itself.
    SameSpeaker(SpeakerId),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownSpeaker(id) => write!(f, "no speaker with id {}", id.0),
            Self::SameSpeaker(id) => write!(f, "speaker {} cannot be merged into itself", id.0),
        }
    }
}

impl std::error::Error for EditError {}

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
    /// The speaker with this id, if listed.
    pub fn speaker(&self, id: SpeakerId) -> Option<&Speaker> {
        self.speakers.iter().find(|s| s.id == id)
    }

    fn speaker_mut(&mut self, id: SpeakerId) -> Result<&mut Speaker, EditError> {
        self.speakers
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(EditError::UnknownSpeaker(id))
    }

    /// Names a speaker; a blank name makes it unnamed again.
    pub fn rename_speaker(&mut self, id: SpeakerId, name: &str) -> Result<(), EditError> {
        let name = name.trim();
        self.speaker_mut(id)?.name = (!name.is_empty()).then(|| name.to_owned());
        Ok(())
    }

    /// Moves every segment of `from` to `into` and removes `from`, for when
    /// diarization split one person in two. `into` keeps its name, or takes
    /// `from`'s when it has none.
    pub fn merge_speakers(&mut self, from: SpeakerId, into: SpeakerId) -> Result<(), EditError> {
        if from == into {
            return Err(EditError::SameSpeaker(from));
        }
        self.speaker_mut(into)?;
        let from_name = self.speaker_mut(from)?.name.take();
        let target = self.speaker_mut(into)?;
        if target.name.is_none() {
            target.name = from_name;
        }
        self.speakers.retain(|s| s.id != from);
        for seg in &mut self.segments {
            if seg.speaker == Some(from) {
                seg.speaker = Some(into);
            }
        }
        Ok(())
    }

    /// Pretty-printed JSON, so the file diffs well and can be read by hand.
    pub fn to_json(&self) -> String {
        // Serializing plain structs with string keys cannot fail.
        serde_json::to_string_pretty(self).expect("transcript serializes")
    }

    /// Reads a transcript, refusing layouts newer than this build knows.
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
    fn rename_and_unname() {
        let mut t = sample();
        t.rename_speaker(SpeakerId(1), "  Anna ").unwrap();
        assert_eq!(
            t.speaker(SpeakerId(1)).unwrap().name.as_deref(),
            Some("Anna")
        );
        t.rename_speaker(SpeakerId(1), " ").unwrap();
        assert_eq!(t.speaker(SpeakerId(1)).unwrap().name, None);
        let err = t.rename_speaker(SpeakerId(9), "X").unwrap_err();
        assert_eq!(err, EditError::UnknownSpeaker(SpeakerId(9)));
        assert_eq!(err.to_string(), "no speaker with id 9");
    }

    #[test]
    fn merge_moves_segments_and_keeps_a_name() {
        let mut t = sample();
        t.segments[1].speaker = Some(SpeakerId(1));
        // Unnamed 1 into named 0: 0 keeps "Ruben".
        t.merge_speakers(SpeakerId(1), SpeakerId(0)).unwrap();
        assert_eq!(t.speakers.len(), 1);
        assert!(t.segments.iter().all(|s| s.speaker == Some(SpeakerId(0))));
        assert_eq!(
            t.speaker(SpeakerId(0)).unwrap().name.as_deref(),
            Some("Ruben")
        );

        // Named into unnamed: the name moves along.
        let mut t = sample();
        t.merge_speakers(SpeakerId(0), SpeakerId(1)).unwrap();
        assert_eq!(
            t.speaker(SpeakerId(1)).unwrap().name.as_deref(),
            Some("Ruben")
        );
        assert_eq!(t.segments[0].speaker, Some(SpeakerId(1)));
        assert_eq!(t.segments[1].speaker, None);
    }

    #[test]
    fn merge_refuses_bad_ids_without_changes() {
        let mut t = sample();
        let err = t.merge_speakers(SpeakerId(0), SpeakerId(0)).unwrap_err();
        assert_eq!(err.to_string(), "speaker 0 cannot be merged into itself");
        assert_eq!(
            t.merge_speakers(SpeakerId(5), SpeakerId(0)),
            Err(EditError::UnknownSpeaker(SpeakerId(5)))
        );
        // A named `from` keeps its name when `into` does not exist.
        assert_eq!(
            t.merge_speakers(SpeakerId(0), SpeakerId(5)),
            Err(EditError::UnknownSpeaker(SpeakerId(5)))
        );
        assert_eq!(t, sample());
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
