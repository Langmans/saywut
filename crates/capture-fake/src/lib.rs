//! A live session replayed from a WAV file and its reference transcript, so
//! the UI can be built and tested without a microphone or a speech model.
//! The replay is a precomputed timeline of [`LiveEvent`]s (levels, growing
//! partial lines, final lines) played against a [`Clock`]; see the README.

use std::{
    ops::ControlFlow,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use saywut_capture_file::{WavError, read_wav};
use saywut_core::{
    asr::Audio,
    live::{LiveEvent, PartialLine, Source, SourceId, SourceKind},
    transcript::{Segment, Speaker, Transcript, TranscriptError},
};

/// Length of the audio each level event is measured over.
pub const FRAME_MS: u64 = 50;

/// Waits until a point in session time.
pub trait Clock {
    /// Returns once session time `at_ms` has come; at once if it has passed.
    fn wait_until(&mut self, at_ms: u64);
}

/// Wall-clock time since the clock was created.
pub struct RealClock {
    start: Instant,
}

impl RealClock {
    /// A clock whose session time starts now.
    pub fn start() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Clock for RealClock {
    fn wait_until(&mut self, at_ms: u64) {
        if let Some(rest) = Duration::from_millis(at_ms).checked_sub(self.start.elapsed()) {
            thread::sleep(rest);
        }
    }
}

/// Never waits: replays as fast as the receiver takes events.
pub struct NoWait;

impl Clock for NoWait {
    fn wait_until(&mut self, _at_ms: u64) {}
}

/// Why a clip could not be opened.
#[derive(Debug)]
pub enum FakeError {
    /// The WAV file.
    Wav(WavError),
    /// Reading the transcript JSON next to it.
    Read(std::io::Error),
    /// Parsing that transcript.
    Transcript(TranscriptError),
}

impl std::fmt::Display for FakeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Wav(e) => write!(f, "cannot read audio: {e}"),
            Self::Read(e) => write!(f, "cannot read transcript: {e}"),
            Self::Transcript(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for FakeError {}

/// A live session replayed from a clip and its transcript.
pub struct FakeSession {
    speakers: Vec<Speaker>,
    timeline: Vec<(u64, LiveEvent)>,
}

impl FakeSession {
    /// Replays `wav` with the transcript JSON next to it (same name, `.json`).
    pub fn open(wav: &Path) -> Result<Self, FakeError> {
        let audio = read_wav(wav).map_err(FakeError::Wav)?;
        let json = std::fs::read_to_string(wav.with_extension("json")).map_err(FakeError::Read)?;
        let transcript = Transcript::from_json(&json).map_err(FakeError::Transcript)?;
        let name = wav
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let source = Source {
            id: SourceId(0),
            kind: SourceKind::File,
            name,
        };
        Ok(Self::new(source, &audio, &transcript))
    }

    /// Replays `audio` as `source`, with `transcript`'s segments as its lines.
    pub fn new(source: Source, audio: &Audio, transcript: &Transcript) -> Self {
        Self {
            speakers: speakers(transcript),
            timeline: timeline(source, audio, &transcript.segments),
        }
    }

    /// Every speaker in the transcript: the listed ones, plus any that only
    /// appear on a segment (without a name), sorted by id.
    pub fn speakers(&self) -> &[Speaker] {
        &self.speakers
    }

    /// The events with the session time they are due at, in order.
    pub fn timeline(&self) -> &[(u64, LiveEvent)] {
        &self.timeline
    }

    /// Plays the timeline, waiting on `clock` before each event. When `emit`
    /// breaks, the replay ends early with a `Stopped` at that event's time.
    pub fn run(&self, clock: &mut dyn Clock, mut emit: impl FnMut(&LiveEvent) -> ControlFlow<()>) {
        for (at_ms, event) in &self.timeline {
            clock.wait_until(*at_ms);
            if emit(event).is_break() && !matches!(event, LiveEvent::Stopped { .. }) {
                let _ = emit(&LiveEvent::Stopped { at_ms: *at_ms });
                return;
            }
        }
    }
}

fn speakers(transcript: &Transcript) -> Vec<Speaker> {
    let mut all = transcript.speakers.clone();
    for id in transcript.segments.iter().filter_map(|s| s.speaker) {
        if !all.iter().any(|s| s.id == id) {
            all.push(Speaker { id, name: None });
        }
    }
    all.sort_by_key(|s| s.id);
    all
}

fn timeline(source: Source, audio: &Audio, segments: &[Segment]) -> Vec<(u64, LiveEvent)> {
    let id = source.id;
    let mut events = vec![(
        0,
        LiveEvent::Started {
            sources: vec![source],
        },
    )];
    events.extend(levels(id, audio));
    for segment in segments {
        events.extend(lines(id, segment));
    }
    // Stable, so events due at the same time keep the order they were added.
    events.sort_by_key(|(at, _)| *at);
    let end = events
        .last()
        .map_or(0, |(at, _)| *at)
        .max(audio.duration_ms());
    events.push((end, LiveEvent::Stopped { at_ms: end }));
    events
}

fn levels(source: SourceId, audio: &Audio) -> Vec<(u64, LiveEvent)> {
    let rate = u64::from(audio.sample_rate);
    if rate == 0 {
        return Vec::new();
    }
    let frame = usize::try_from(rate * FRAME_MS / 1000)
        .unwrap_or(usize::MAX)
        .max(1);
    let mut end = 0;
    audio
        .samples
        .chunks(frame)
        .map(|chunk| {
            end += chunk.len() as u64;
            let at_ms = end * 1000 / rate;
            (at_ms, LiveEvent::level(source, at_ms, chunk))
        })
        .collect()
}

/// Partial lines with 1..=n words spread evenly over the segment, then the
/// final line. A segment that ends before it starts is final at its start.
fn lines(source: SourceId, segment: &Segment) -> Vec<(u64, LiveEvent)> {
    let words: Vec<&str> = segment.text.split_whitespace().collect();
    let span = u128::from(segment.end_ms.saturating_sub(segment.start_ms));
    let steps = words.len() as u128 + 1;
    let mut out: Vec<(u64, LiveEvent)> = (1..=words.len())
        .map(|k| {
            // At most `span`, so the cast back cannot truncate.
            let offset = (span * k as u128 / steps) as u64;
            let line = PartialLine {
                start_ms: segment.start_ms,
                speaker: segment.speaker,
                text: words[..k].join(" "),
            };
            (
                segment.start_ms + offset,
                LiveEvent::Partial { source, line },
            )
        })
        .collect();
    out.push((
        segment.end_ms.max(segment.start_ms),
        LiveEvent::Final {
            source,
            segment: segment.clone(),
        },
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use saywut_core::transcript::SpeakerId;

    fn source() -> Source {
        Source {
            id: SourceId(3),
            kind: SourceKind::File,
            name: "clip.wav".into(),
        }
    }

    fn seg(start_ms: u64, end_ms: u64, speaker: Option<u32>, text: &str) -> Segment {
        Segment {
            start_ms,
            end_ms,
            speaker: speaker.map(SpeakerId),
            text: text.into(),
        }
    }

    /// `ms` milliseconds of audio at 1 kHz, so one sample is one millisecond.
    fn audio(ms: usize) -> Audio {
        Audio {
            sample_rate: 1000,
            samples: vec![0.5; ms],
        }
    }

    fn transcript(segments: Vec<Segment>) -> Transcript {
        Transcript {
            segments,
            ..Transcript::default()
        }
    }

    fn times(session: &FakeSession) -> Vec<u64> {
        session.timeline().iter().map(|(at, _)| *at).collect()
    }

    /// Partial texts and final texts with their times, levels left out.
    fn lines_of(session: &FakeSession) -> Vec<(u64, String)> {
        session
            .timeline()
            .iter()
            .filter_map(|(at, e)| match e {
                LiveEvent::Partial { line, .. } => Some((*at, format!("~{}", line.text))),
                LiveEvent::Final { segment, .. } => Some((*at, segment.text.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn starts_first_and_stops_last() {
        let s = FakeSession::new(source(), &audio(120), &transcript(vec![]));
        let tl = s.timeline();
        assert_eq!(
            tl[0],
            (
                0,
                LiveEvent::Started {
                    sources: vec![source()]
                }
            )
        );
        assert_eq!(
            tl.last().unwrap(),
            &(120, LiveEvent::Stopped { at_ms: 120 })
        );
    }

    #[test]
    fn levels_every_frame_including_a_short_last_one() {
        let s = FakeSession::new(source(), &audio(120), &transcript(vec![]));
        assert_eq!(times(&s), vec![0, 50, 100, 120, 120]);
        assert_eq!(
            s.timeline()[1].1,
            LiveEvent::level(SourceId(3), 50, &[0.5; 50])
        );
    }

    #[test]
    fn no_levels_without_a_sample_rate() {
        let broken = Audio {
            sample_rate: 0,
            samples: vec![0.0; 10],
        };
        let s = FakeSession::new(source(), &broken, &transcript(vec![]));
        assert_eq!(times(&s), vec![0, 0]);
    }

    #[test]
    fn words_grow_then_become_final_at_the_end() {
        let s = FakeSession::new(
            source(),
            &audio(0),
            &transcript(vec![seg(100, 400, Some(1), "one two three")]),
        );
        assert_eq!(
            lines_of(&s),
            vec![
                (175, "~one".into()),
                (250, "~one two".into()),
                (325, "~one two three".into()),
                (400, "one two three".into()),
            ]
        );
        assert_eq!(
            s.timeline()[1].1,
            LiveEvent::Partial {
                source: SourceId(3),
                line: PartialLine {
                    start_ms: 100,
                    speaker: Some(SpeakerId(1)),
                    text: "one".into(),
                },
            }
        );
    }

    #[test]
    fn stops_after_the_last_line_when_audio_is_shorter() {
        let s = FakeSession::new(
            source(),
            &audio(0),
            &transcript(vec![seg(0, 500, None, "")]),
        );
        assert_eq!(lines_of(&s), vec![(500, String::new())]);
        assert_eq!(times(&s).last(), Some(&500));
    }

    #[test]
    fn backwards_segment_is_final_at_its_start() {
        let s = FakeSession::new(
            source(),
            &audio(0),
            &transcript(vec![seg(300, 100, None, "a b")]),
        );
        assert_eq!(
            lines_of(&s),
            vec![
                (300, "~a".into()),
                (300, "~a b".into()),
                (300, "a b".into())
            ]
        );
    }

    #[test]
    fn huge_times_do_not_overflow() {
        let s = FakeSession::new(
            source(),
            &audio(0),
            &transcript(vec![seg(0, u64::MAX, None, "a b c d e")]),
        );
        assert_eq!(times(&s).last(), Some(&u64::MAX));
    }

    #[test]
    fn segments_interleave_with_levels_in_time_order() {
        let s = FakeSession::new(
            source(),
            &audio(100),
            &transcript(vec![seg(60, 80, None, "hi")]),
        );
        let t = times(&s);
        assert!(t.windows(2).all(|w| w[0] <= w[1]), "{t:?}");
        assert_eq!(t, vec![0, 50, 70, 80, 100, 100]);
    }

    #[test]
    fn speakers_include_unlisted_ids() {
        let mut t = transcript(vec![
            seg(0, 1, Some(2), "a"),
            seg(1, 2, None, "b"),
            seg(2, 3, Some(5), "c"),
            seg(3, 4, Some(2), "d"),
        ]);
        t.speakers = vec![Speaker {
            id: SpeakerId(5),
            name: Some("Zira".into()),
        }];
        let s = FakeSession::new(source(), &audio(0), &t);
        assert_eq!(
            s.speakers(),
            &[
                Speaker {
                    id: SpeakerId(2),
                    name: None
                },
                Speaker {
                    id: SpeakerId(5),
                    name: Some("Zira".into())
                },
            ]
        );
    }

    struct Recorder(Vec<u64>);

    impl Clock for Recorder {
        fn wait_until(&mut self, at_ms: u64) {
            self.0.push(at_ms);
        }
    }

    #[test]
    fn run_waits_for_each_event() {
        let s = FakeSession::new(source(), &audio(100), &transcript(vec![]));
        let mut clock = Recorder(Vec::new());
        let mut seen = Vec::new();
        s.run(&mut clock, |e| {
            seen.push(e.clone());
            ControlFlow::Continue(())
        });
        assert_eq!(clock.0, times(&s));
        assert_eq!(seen.len(), s.timeline().len());
    }

    #[test]
    fn breaking_stops_at_that_time() {
        let s = FakeSession::new(source(), &audio(200), &transcript(vec![]));
        let mut seen = Vec::new();
        s.run(&mut NoWait, |e| {
            seen.push(e.clone());
            if seen.len() == 3 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        });
        assert_eq!(seen.len(), 4);
        assert_eq!(seen[3], LiveEvent::Stopped { at_ms: 100 });
    }

    #[test]
    fn breaking_on_stopped_sends_it_once() {
        let s = FakeSession::new(source(), &audio(0), &transcript(vec![]));
        let mut seen = 0;
        s.run(&mut NoWait, |_| {
            seen += 1;
            ControlFlow::Break(())
        });
        // Breaking on Started sends Stopped; nothing else follows.
        assert_eq!(seen, 2);
        let mut stops = 0;
        let only_stop = FakeSession {
            speakers: Vec::new(),
            timeline: vec![(0, LiveEvent::Stopped { at_ms: 0 })],
        };
        only_stop.run(&mut NoWait, |_| {
            stops += 1;
            ControlFlow::Break(())
        });
        assert_eq!(stops, 1);
    }

    #[test]
    fn real_clock_waits_and_never_goes_back() {
        let mut clock = RealClock::start();
        clock.wait_until(5);
        assert!(clock.start.elapsed() >= Duration::from_millis(5));
        // Already past: returns at once.
        clock.wait_until(0);
    }

    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("saywut-fake-{}-{name}", std::process::id()))
    }

    fn write_wav(path: &Path) {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 1000,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut w = hound::WavWriter::create(path, spec).unwrap();
        for _ in 0..100 {
            w.write_sample(16384_i16).unwrap();
        }
        w.finalize().unwrap();
    }

    #[test]
    fn opens_a_wav_with_its_transcript() {
        let wav = temp("ok.wav");
        write_wav(&wav);
        let json =
            r#"{"version":1,"segments":[{"start_ms":0,"end_ms":90,"speaker":1,"text":"hi"}]}"#;
        std::fs::write(wav.with_extension("json"), json).unwrap();
        let s = FakeSession::open(&wav).unwrap();
        std::fs::remove_file(&wav).unwrap();
        std::fs::remove_file(wav.with_extension("json")).unwrap();
        assert_eq!(
            s.timeline()[0].1,
            LiveEvent::Started {
                sources: vec![Source {
                    id: SourceId(0),
                    kind: SourceKind::File,
                    name: wav.file_name().unwrap().to_string_lossy().into_owned(),
                }]
            }
        );
        assert_eq!(s.speakers()[0].id, SpeakerId(1));
        assert_eq!(times(&s).last(), Some(&100));
    }

    #[test]
    fn missing_wav_is_an_error() {
        let err = FakeSession::open(Path::new("no/such/clip.wav"))
            .err()
            .unwrap();
        assert!(matches!(err, FakeError::Wav(_)));
        assert!(err.to_string().starts_with("cannot read audio"));
    }

    #[test]
    fn missing_transcript_is_an_error() {
        let wav = temp("nojson.wav");
        write_wav(&wav);
        let err = FakeSession::open(&wav).err().unwrap();
        std::fs::remove_file(&wav).unwrap();
        assert!(matches!(err, FakeError::Read(_)));
        assert!(err.to_string().starts_with("cannot read transcript"));
    }

    #[test]
    fn malformed_transcript_is_an_error() {
        let wav = temp("bad.wav");
        write_wav(&wav);
        std::fs::write(wav.with_extension("json"), "{").unwrap();
        let err = FakeSession::open(&wav).err().unwrap();
        std::fs::remove_file(&wav).unwrap();
        std::fs::remove_file(wav.with_extension("json")).unwrap();
        assert!(matches!(err, FakeError::Transcript(_)));
        assert!(err.to_string().starts_with("invalid transcript JSON"));
    }
}
