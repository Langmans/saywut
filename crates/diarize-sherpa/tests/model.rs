//! Tests against the real diarization models in the repository's `models/`
//! (`saywut models fetch pyannote-segmentation-3` and
//! `saywut models fetch campplus-voxceleb`) and the TTS clips in
//! `testdata/tts/` (`tools/tts-clips.ps1`). Missing files fail the tests.

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

/// Share of the clip's reference lines that get their true speaker when the
/// diarizer's turns are applied to the reference timing.
fn speaker_accuracy(d: &mut SherpaDiarizer, clip: &str) -> (f64, usize) {
    let base = repo().join("testdata/tts").join(clip);
    let audio = saywut_capture_file::read_wav(&base.with_extension("wav")).unwrap();
    let reference =
        Transcript::from_json(&std::fs::read_to_string(base.with_extension("json")).unwrap())
            .unwrap();
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
        "{clip}: {speakers} speakers, {right}/{} lines right",
        segments.len()
    );
    (accuracy, speakers as usize)
}

#[test]
fn two_voices_are_told_apart() {
    let (accuracy, speakers) = speaker_accuracy(&mut diarizer(None), "en-dialogue");
    assert_eq!(speakers, 2);
    assert_eq!(accuracy, 1.0);
}

/// en-three-speakers uses David, Zira and Mark. David and Mark are two very
/// similar male Microsoft voices that the embedding model cannot tell apart,
/// even with the speaker count given: their four lines end up as one
/// speaker. What the clip can show is that Zira stays separate and that the
/// count is passed on without breaking anything; real three-speaker accuracy
/// is measured on recorded speech (see PLAN.md, phase 1).
#[test]
fn similar_synthetic_voices_merge_distinct_ones_do_not() {
    let (accuracy, speakers) = speaker_accuracy(&mut diarizer(Some(3)), "en-three-speakers");
    assert!(speakers >= 2, "{speakers}");
    assert!(accuracy >= 4.0 / 6.0, "{accuracy}");
}

/// One line of en-dialogue as its own audio.
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
    let base = repo().join("testdata/tts/en-dialogue");
    let audio = saywut_capture_file::read_wav(&base.with_extension("wav")).unwrap();
    let reference =
        Transcript::from_json(&std::fs::read_to_string(base.with_extension("json")).unwrap())
            .unwrap();
    let mut e = SherpaEmbedder::load(&model("campplus-voxceleb")).unwrap();
    let mut embed = |i| e.embed(&line(&audio, &reference, i)).unwrap();
    // Lines 0 and 2 are David, line 1 is Zira.
    let (david, david_again, zira) = (embed(0), embed(2), embed(1));
    let same = cosine(&david, &david_again);
    let other = cosine(&david, &zira);
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
