//! Tests against the real diarization models in the repository's `models/`
//! (`saywut models fetch pyannote-segmentation-3` and
//! `saywut models fetch campplus-voxceleb`) and the TTS clips in
//! `testdata/tts/` (`tools/tts-clips.ps1`). Missing files fail the tests.

use std::path::{Path, PathBuf};

use saywut_core::{
    asr::Audio,
    diarize::{Diarizer, assign_speakers, normalize},
    transcript::Transcript,
};
use saywut_diarize_sherpa::SherpaDiarizer;
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
