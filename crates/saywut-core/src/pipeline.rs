//! Whole-file transcription: audio in, [`Transcript`] out.

use std::collections::BTreeSet;

use crate::asr::{AsrEngine, AsrError, Audio};
use crate::diarize::{Turn, assign_speakers, normalize};
use crate::transcript::{Segment, Speaker, Transcript};

/// Every speaker that occurs in a segment, in id order and unnamed.
fn speakers_of(segments: &[Segment]) -> Vec<Speaker> {
    let ids: BTreeSet<_> = segments.iter().filter_map(|s| s.speaker).collect();
    ids.into_iter()
        .map(|id| Speaker { id, name: None })
        .collect()
}

/// Labels the transcript's segments with the diarizer's turns. Earlier labels
/// and names are replaced: a new diarization numbers speakers afresh, so an
/// old name could land on someone else.
pub fn apply_diarization(transcript: &mut Transcript, turns: Vec<Turn>) {
    assign_speakers(&mut transcript.segments, &normalize(turns));
    transcript.speakers = speakers_of(&transcript.segments);
}

/// Runs `engine` over all of `audio`. Segments with only whitespace (engines
/// emit them for silence) are dropped. The speaker list holds every speaker
/// that occurs in a segment, in id order and still unnamed.
pub fn transcribe_file(
    engine: &mut dyn AsrEngine,
    audio: &Audio,
    language: Option<&str>,
) -> Result<Transcript, AsrError> {
    let mut segments = engine.transcribe(audio, language)?;
    segments.retain(|s| !s.text.trim().is_empty());
    Ok(Transcript {
        language: language.map(str::to_owned),
        speakers: speakers_of(&segments),
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
    fn drops_blank_segments() {
        let blank = Segment {
            text: " \n".into(),
            ..seg(500, Some(1))
        };
        let mut engine = FakeAsr {
            languages: vec![],
            script: vec![seg(0, Some(0)), blank],
        };
        let t = transcribe_file(&mut engine, &audio(), None).unwrap();
        assert_eq!(t.segments, vec![seg(0, Some(0))]);
        // A speaker heard only in a blank segment is not listed.
        assert_eq!(t.speakers.len(), 1);
    }

    #[test]
    fn diarization_relabels_and_forgets_old_names() {
        let mut t = Transcript {
            speakers: vec![Speaker {
                id: SpeakerId(0),
                name: Some("Old".into()),
            }],
            segments: vec![seg(0, Some(0)), seg(1000, Some(0)), seg(5000, Some(0))],
            ..Transcript::default()
        };
        let turns = vec![
            Turn {
                start_ms: 900,
                end_ms: 1200,
                speaker: SpeakerId(4),
            },
            Turn {
                start_ms: 0,
                end_ms: 150,
                speaker: SpeakerId(9),
            },
        ];
        apply_diarization(&mut t, turns);
        let labels: Vec<_> = t.segments.iter().map(|s| s.speaker).collect();
        // 9 speaks first and becomes 0; the segment at 5 s is too far from any turn.
        assert_eq!(labels, vec![Some(SpeakerId(0)), Some(SpeakerId(1)), None]);
        assert_eq!(
            t.speakers,
            vec![
                Speaker {
                    id: SpeakerId(0),
                    name: None
                },
                Speaker {
                    id: SpeakerId(1),
                    name: None
                }
            ]
        );
    }

    #[test]
    fn passes_engine_errors_through() {
        let mut engine = FakeAsr::default();
        let err = transcribe_file(&mut engine, &audio(), Some("nl")).unwrap_err();
        assert_eq!(err, AsrError::UnsupportedLanguage("nl".into()));
    }
}
