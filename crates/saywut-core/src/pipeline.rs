//! Whole-file transcription: audio in, [`Transcript`] out.

use std::collections::BTreeSet;

use crate::asr::{AsrEngine, AsrError, Audio};
use crate::transcript::{Speaker, Transcript};

/// Runs `engine` over all of `audio`. The speaker list holds every speaker
/// that occurs in a segment, in id order and still unnamed.
pub fn transcribe_file(
    engine: &mut dyn AsrEngine,
    audio: &Audio,
    language: Option<&str>,
) -> Result<Transcript, AsrError> {
    let segments = engine.transcribe(audio, language)?;
    let ids: BTreeSet<_> = segments.iter().filter_map(|s| s.speaker).collect();
    Ok(Transcript {
        language: language.map(str::to_owned),
        speakers: ids
            .into_iter()
            .map(|id| Speaker { id, name: None })
            .collect(),
        segments,
        ..Transcript::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::FakeAsr;
    use crate::transcript::{Segment, SpeakerId};

    fn seg(start_ms: u64, speaker: Option<u32>) -> Segment {
        Segment {
            start_ms,
            end_ms: start_ms + 100,
            speaker: speaker.map(SpeakerId),
            text: "x".into(),
        }
    }

    fn audio() -> Audio {
        Audio {
            sample_rate: 1000,
            samples: vec![0.0; 1000],
        }
    }

    #[test]
    fn collects_speakers_in_id_order() {
        let mut engine = FakeAsr {
            languages: vec!["en".into()],
            script: vec![
                seg(0, Some(2)),
                seg(100, None),
                seg(200, Some(0)),
                seg(300, Some(2)),
            ],
        };
        let t = transcribe_file(&mut engine, &audio(), Some("en")).unwrap();
        assert_eq!(t.language.as_deref(), Some("en"));
        assert_eq!(
            t.speakers,
            vec![
                Speaker {
                    id: SpeakerId(0),
                    name: None
                },
                Speaker {
                    id: SpeakerId(2),
                    name: None
                }
            ]
        );
        assert_eq!(t.segments.len(), 4);
    }

    #[test]
    fn passes_engine_errors_through() {
        let mut engine = FakeAsr::default();
        let err = transcribe_file(&mut engine, &audio(), Some("nl")).unwrap_err();
        assert_eq!(err, AsrError::UnsupportedLanguage("nl".into()));
    }
}
