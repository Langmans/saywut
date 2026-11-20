//! Speaker diarization port: who speaks when.
//!
//! A [`Diarizer`] returns speaker turns for a whole recording; ASR segments
//! get their speaker from those turns with [`assign_speakers`]. Live labels
//! are provisional; after Stop the whole recording is diarized again and the
//! labels are replaced.

use crate::asr::Audio;
use crate::transcript::{Segment, SpeakerId};

/// A stretch of audio in which one speaker talks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Turn {
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: SpeakerId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiarizeError(pub String);

impl std::fmt::Display for DiarizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "diarization failed: {}", self.0)
    }
}

impl std::error::Error for DiarizeError {}

pub trait Diarizer {
    /// Speaker turns over all of `audio`, ordered by start. Speaker ids are
    /// numbered from 0 in order of first appearance.
    fn diarize(&mut self, audio: &Audio) -> Result<Vec<Turn>, DiarizeError>;
}

/// Returns scripted turns, clipped to the length of the audio.
#[derive(Debug, Clone, Default)]
pub struct FakeDiarizer {
    pub turns: Vec<Turn>,
}

impl Diarizer for FakeDiarizer {
    fn diarize(&mut self, audio: &Audio) -> Result<Vec<Turn>, DiarizeError> {
        let end = audio.duration_ms();
        Ok(self
            .turns
            .iter()
            .filter(|t| t.start_ms < end)
            .map(|t| Turn {
                end_ms: t.end_ms.min(end),
                ..*t
            })
            .collect())
    }
}

/// Sorts turns by start and renumbers speakers from 0 in order of first
/// appearance, so "Speaker 1" is whoever speaks first whatever ids a
/// diarizer's clustering produced.
pub fn normalize(mut turns: Vec<Turn>) -> Vec<Turn> {
    turns.sort_by_key(|t| (t.start_ms, t.end_ms));
    let mut seen: Vec<SpeakerId> = Vec::new();
    for t in &mut turns {
        let index = match seen.iter().position(|s| *s == t.speaker) {
            Some(i) => i,
            None => {
                seen.push(t.speaker);
                seen.len() - 1
            }
        };
        t.speaker = SpeakerId(index as u32);
    }
    turns
}

/// How far a segment may lie from the nearest turn and still get its speaker.
/// ASR and diarization cut speech slightly differently, so a short segment can
/// fall in a gap between turns.
pub const MAX_GAP_MS: u64 = 1000;

fn overlap(a: (u64, u64), b: (u64, u64)) -> u64 {
    a.1.min(b.1).saturating_sub(a.0.max(b.0))
}

/// Gap between two spans; 0 when they touch or overlap.
fn distance(a: (u64, u64), b: (u64, u64)) -> u64 {
    b.0.saturating_sub(a.1).max(a.0.saturating_sub(b.1))
}

/// Gives every segment the speaker it overlaps most. A segment that overlaps
/// no turn takes the nearest turn's speaker if it is within [`MAX_GAP_MS`],
/// else none. Ties go to the earlier turn.
pub fn assign_speakers(segments: &mut [Segment], turns: &[Turn]) {
    for seg in segments {
        let span = (seg.start_ms, seg.end_ms);
        let best = turns
            .iter()
            .map(|t| (t, overlap(span, (t.start_ms, t.end_ms))))
            .filter(|(_, o)| *o > 0)
            .fold(None, |best: Option<(&Turn, u64)>, (t, o)| match best {
                Some((_, b)) if b >= o => best,
                _ => Some((t, o)),
            })
            .map(|(t, _)| t.speaker);
        seg.speaker = best.or_else(|| {
            turns
                .iter()
                .map(|t| (t, distance(span, (t.start_ms, t.end_ms))))
                .filter(|(_, d)| *d <= MAX_GAP_MS)
                .min_by_key(|(_, d)| *d)
                .map(|(t, _)| t.speaker)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turn(start_ms: u64, end_ms: u64, speaker: u32) -> Turn {
        Turn {
            start_ms,
            end_ms,
            speaker: SpeakerId(speaker),
        }
    }

    fn seg(start_ms: u64, end_ms: u64) -> Segment {
        Segment {
            start_ms,
            end_ms,
            speaker: None,
            text: String::new(),
        }
    }

    fn speakers(segments: &[Segment]) -> Vec<Option<u32>> {
        segments.iter().map(|s| s.speaker.map(|id| id.0)).collect()
    }

    #[test]
    fn normalize_sorts_and_numbers_by_first_appearance() {
        let turns = vec![turn(2000, 3000, 7), turn(0, 1000, 3), turn(1000, 2000, 7)];
        assert_eq!(
            normalize(turns),
            vec![turn(0, 1000, 0), turn(1000, 2000, 1), turn(2000, 3000, 1)]
        );
        assert!(normalize(vec![]).is_empty());
    }

    #[test]
    fn largest_overlap_wins() {
        let turns = [turn(0, 1000, 0), turn(1000, 3000, 1)];
        let mut segs = vec![seg(0, 900), seg(800, 2000), seg(2500, 3000)];
        assign_speakers(&mut segs, &turns);
        assert_eq!(speakers(&segs), vec![Some(0), Some(1), Some(1)]);
    }

    #[test]
    fn tie_goes_to_the_earlier_turn() {
        let turns = [turn(0, 1000, 0), turn(1000, 2000, 1)];
        let mut segs = vec![seg(500, 1500)];
        assign_speakers(&mut segs, &turns);
        assert_eq!(speakers(&segs), vec![Some(0)]);
    }

    #[test]
    fn gap_segments_take_the_nearest_turn_within_limit() {
        let turns = [turn(0, 1000, 0), turn(5000, 6000, 1)];
        let mut segs = vec![
            seg(1200, 1500),              // 200 ms after turn 0
            seg(4500, 4800),              // 200 ms before turn 1
            seg(2500, 3000),              // 1500 ms from both: too far
            seg(6000 + MAX_GAP_MS, 7500), // exactly at the limit
        ];
        assign_speakers(&mut segs, &turns);
        assert_eq!(speakers(&segs), vec![Some(0), Some(1), None, Some(1)]);
    }

    #[test]
    fn no_turns_clears_speakers() {
        let mut segs = vec![Segment {
            speaker: Some(SpeakerId(3)),
            ..seg(0, 100)
        }];
        assign_speakers(&mut segs, &[]);
        assert_eq!(speakers(&segs), vec![None]);
    }

    #[test]
    fn fake_diarizer_clips_to_audio() {
        let mut d = FakeDiarizer {
            turns: vec![turn(0, 800, 0), turn(800, 2000, 1), turn(3000, 4000, 0)],
        };
        let audio = Audio {
            sample_rate: 1000,
            samples: vec![0.0; 1500],
        };
        assert_eq!(
            d.diarize(&audio).unwrap(),
            vec![turn(0, 800, 0), turn(800, 1500, 1)]
        );
    }

    #[test]
    fn error_message() {
        assert_eq!(
            DiarizeError("no model".into()).to_string(),
            "diarization failed: no model"
        );
    }
}
