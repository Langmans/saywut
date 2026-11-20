//! Exports derived from a [`Transcript`]: SRT, WebVTT and plain text.
//!
//! The core holds no UI strings. An unnamed speaker gets its label from the
//! caller's `fallback`, which the CLI and UI fill from the Fluent locale
//! ("Speaker 1", "Spreker 1", "Sprecher 1").

use crate::transcript::{Segment, SpeakerId, Transcript};

/// Label for a speaker without a name.
pub type Fallback<'a> = &'a dyn Fn(SpeakerId) -> String;

fn label(t: &Transcript, seg: &Segment, fallback: Fallback) -> Option<String> {
    let id = seg.speaker?;
    Some(match t.speaker(id).and_then(|s| s.name.clone()) {
        Some(name) => name,
        None => fallback(id),
    })
}

/// `HH:MM:SS` plus milliseconds after `sep` (`,` for SRT, `.` for VTT).
/// Hours are not capped at 99, so long recordings stay correct.
fn timestamp(ms: u64, sep: char) -> String {
    let (h, rest) = (ms / 3_600_000, ms % 3_600_000);
    let (m, rest) = (rest / 60_000, rest % 60_000);
    let (s, ms) = (rest / 1000, rest % 1000);
    format!("{h:02}:{m:02}:{s:02}{sep}{ms:03}")
}

/// SubRip. Speakers are a `Name: ` prefix, as SRT has no voice markup.
pub fn to_srt(t: &Transcript, fallback: Fallback) -> String {
    let mut out = String::new();
    for (i, seg) in t.segments.iter().enumerate() {
        let text = match label(t, seg, fallback) {
            Some(name) => format!("{name}: {}", seg.text),
            None => seg.text.clone(),
        };
        out.push_str(&format!(
            "{}\n{} --> {}\n{text}\n\n",
            i + 1,
            timestamp(seg.start_ms, ','),
            timestamp(seg.end_ms, ',')
        ));
    }
    out
}

/// WebVTT with `<v Name>` voice spans, which players can style per speaker.
pub fn to_vtt(t: &Transcript, fallback: Fallback) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for seg in &t.segments {
        let text = escape_vtt(&seg.text);
        let text = match label(t, seg, fallback) {
            Some(name) => format!("<v {}>{text}", escape_vtt(&name)),
            None => text,
        };
        out.push_str(&format!(
            "{} --> {}\n{text}\n\n",
            timestamp(seg.start_ms, '.'),
            timestamp(seg.end_ms, '.')
        ));
    }
    out
}

/// `&`, `<` and `>` would otherwise be read as VTT markup.
fn escape_vtt(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Plain text: one line per segment, start time in brackets. Consecutive
/// segments of the same speaker are not merged, so every line keeps its time.
pub fn to_txt(t: &Transcript, fallback: Fallback) -> String {
    let mut out = String::new();
    for seg in &t.segments {
        let secs = seg.start_ms / 1000;
        let stamp = format!(
            "[{:02}:{:02}:{:02}]",
            secs / 3600,
            secs / 60 % 60,
            secs % 60
        );
        match label(t, seg, fallback) {
            Some(name) => out.push_str(&format!("{stamp} {name}: {}\n", seg.text)),
            None => out.push_str(&format!("{stamp} {}\n", seg.text)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::Speaker;

    fn fallback(id: SpeakerId) -> String {
        format!("Speaker {}", id.0 + 1)
    }

    fn sample() -> Transcript {
        Transcript {
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
                    start_ms: 3_723_004,
                    end_ms: 3_725_000,
                    speaker: Some(SpeakerId(1)),
                    text: "a < b & c".into(),
                },
                Segment {
                    start_ms: 3_726_000,
                    end_ms: 3_727_000,
                    speaker: None,
                    text: "(muziek)".into(),
                },
            ],
            ..Transcript::default()
        }
    }

    #[test]
    fn timestamp_formats() {
        assert_eq!(timestamp(0, ','), "00:00:00,000");
        assert_eq!(timestamp(3_723_004, '.'), "01:02:03.004");
        assert_eq!(timestamp(100 * 3_600_000, ','), "100:00:00,000");
    }

    #[test]
    fn srt() {
        assert_eq!(
            to_srt(&sample(), &fallback),
            "1\n00:00:00,000 --> 00:00:01,500\nRuben: Hallo.\n\n\
             2\n01:02:03,004 --> 01:02:05,000\nSpeaker 2: a < b & c\n\n\
             3\n01:02:06,000 --> 01:02:07,000\n(muziek)\n\n"
        );
    }

    #[test]
    fn vtt() {
        assert_eq!(
            to_vtt(&sample(), &fallback),
            "WEBVTT\n\n\
             00:00:00.000 --> 00:00:01.500\n<v Ruben>Hallo.\n\n\
             01:02:03.004 --> 01:02:05.000\n<v Speaker 2>a &lt; b &amp; c\n\n\
             01:02:06.000 --> 01:02:07.000\n(muziek)\n\n"
        );
    }

    #[test]
    fn vtt_escapes_speaker_name() {
        let mut t = sample();
        t.speakers[0].name = Some("<Ruben>".into());
        assert!(to_vtt(&t, &fallback).contains("<v &lt;Ruben&gt;>Hallo."));
    }

    #[test]
    fn txt() {
        assert_eq!(
            to_txt(&sample(), &fallback),
            "[00:00:00] Ruben: Hallo.\n\
             [01:02:03] Speaker 2: a < b & c\n\
             [01:02:06] (muziek)\n"
        );
    }

    #[test]
    fn unknown_speaker_id_uses_fallback() {
        let mut t = sample();
        t.speakers.clear();
        assert!(to_txt(&t, &fallback).contains("Speaker 1: Hallo."));
    }

    #[test]
    fn empty_transcript() {
        let t = Transcript::default();
        assert_eq!(to_srt(&t, &fallback), "");
        assert_eq!(to_vtt(&t, &fallback), "WEBVTT\n\n");
        assert_eq!(to_txt(&t, &fallback), "");
    }
}
