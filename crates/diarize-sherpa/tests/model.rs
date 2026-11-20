//! Tests against the real diarization models in the repository's `models/`
//! (`saywut models fetch pyannote-segmentation-3` and
//! `saywut models fetch campplus-voxceleb`) and the committed FLEURS clip
//! `testdata/speech/two-voices` (two people, halves alternating A, B, A, B).
//! Missing models fail the tests.

use std::path::{Path, PathBuf};

use saywut_core::{
    asr::Audio,
    diarize::{Diarizer, assign_speakers, normalize},
    profiles::{MATCH_THRESHOLD, SpeakerEmbedder, cosine},
    transcript::Transcript,
};
use saywut_diarize_sherpa::{SherpaDiarizer, SherpaEmbedder};
use saywut_models::Manifest;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn model(id: &str) -> PathBuf {
    let manifest = Manifest::builtin();
    let path = manifest.get(id).unwrap().path(&repo().join("models"));
    assert!(path.is_file(), "model missing: {}", path.display());
    path
}

fn diarizer(speakers: Option<u32>) -> SherpaDiarizer {
    SherpaDiarizer::load(
        &model("pyannote-segmentation-3"),
        &model("campplus-voxceleb"),
        speakers,
    )
    .unwrap()
}

fn two_voices() -> (Audio, Transcript) {
    let base = repo().join("testdata/speech/two-voices");
    let audio = saywut_capture_file::read_wav(&base.with_extension("wav")).unwrap();
    let reference =
        Transcript::from_json(&std::fs::read_to_string(base.with_extension("json")).unwrap())
            .unwrap();
    (audio, reference)
}

/// Share of the clip's reference lines that get their true speaker when the
/// diarizer's turns are applied to the reference timing, and the number of
/// speakers found.
fn speaker_accuracy(d: &mut SherpaDiarizer) -> (f64, usize) {
    let (audio, reference) = two_voices();
    let turns = normalize(d.diarize(&audio).unwrap());
    let speakers = turns.iter().map(|t| t.speaker).max().map_or(0, |s| s.0 + 1);
    let mut segments = reference.segments.clone();
    assign_speakers(&mut segments, &turns);
    let right = segments
        .iter()
        .zip(&reference.segments)
        .filter(|(got, want)| got.speaker == want.speaker)
        .count();
    let accuracy = right as f64 / segments.len() as f64;
    eprintln!(
        "{speakers} speakers, {right}/{} lines right",
        segments.len()
    );
    (accuracy, speakers as usize)
}

#[test]
fn two_voices_are_told_apart() {
    let (accuracy, speakers) = speaker_accuracy(&mut diarizer(None));
    assert_eq!(speakers, 2);
    assert_eq!(accuracy, 1.0);
}

#[test]
fn speaker_count_can_be_given() {
    let (accuracy, speakers) = speaker_accuracy(&mut diarizer(Some(2)));
    assert_eq!((accuracy, speakers), (1.0, 2));
}

/// One line of the clip as its own audio.
fn line(audio: &Audio, reference: &Transcript, index: usize) -> Audio {
    let seg = &reference.segments[index];
    let at = |ms: u64| (ms * 16) as usize;
    Audio {
        sample_rate: audio.sample_rate,
        samples: audio.samples[at(seg.start_ms)..at(seg.end_ms)].to_vec(),
    }
}

#[test]
fn embeddings_match_the_same_voice_only() {
    let (audio, reference) = two_voices();
    let mut e = SherpaEmbedder::load(&model("campplus-voxceleb")).unwrap();
    let mut embed = |i| e.embed(&line(&audio, &reference, i)).unwrap();
    // Lines 0 and 2 are the two halves of voice A, line 1 is voice B.
    let (a, a_again, b) = (embed(0), embed(2), embed(1));
    let same = cosine(&a, &a_again);
    let other = cosine(&a, &b);
    eprintln!("same voice {same:.2}, other voice {other:.2}");
    assert!(same >= MATCH_THRESHOLD, "{same}");
    assert!(other < MATCH_THRESHOLD, "{other}");

    let err = e
        .embed(&Audio {
            sample_rate: 8_000,
            samples: vec![0.0; 8_000],
        })
        .unwrap_err();
    assert!(err.to_string().contains("16000 Hz"), "{err}");
    assert!(SherpaEmbedder::load(Path::new("no/such.onnx")).is_err());
}

#[test]
fn rejects_wrong_rate_and_missing_models() {
    let audio = Audio {
        sample_rate: 44_100,
        samples: vec![0.0; 441],
    };
    let err = diarizer(None).diarize(&audio).unwrap_err();
    assert!(err.to_string().contains("16000 Hz"), "{err}");

    let missing = Path::new("no/such/model.onnx");
    let err = SherpaDiarizer::load(missing, &model("campplus-voxceleb"), None)
        .err()
        .unwrap();
    assert!(err.to_string().contains("model file not usable"), "{err}");
}
