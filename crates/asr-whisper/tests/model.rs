//! Tests against the real whisper-base model in the repository's `models/`;
//! fetch it with `saywut models fetch whisper-base --model-dir models`. A
//! missing model fails the test on purpose: skipping would hide an untested
//! engine.
//!
//! The tests run one at a time: each whisper run uses every core, and ggml's
//! busy-waiting thread pools slow down by an order of magnitude when several
//! compete for the same cores.

use std::{
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, PoisonError},
};

use saywut_asr_whisper::WhisperAsr;
use saywut_core::{
    asr::{AsrEngine, AsrError, Audio},
    transcript::Transcript,
    wer::wer,
};
use saywut_models::Manifest;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// The engine, plus a guard that keeps other model tests waiting until the
/// caller is done.
fn engine() -> (MutexGuard<'static, ()>, WhisperAsr) {
    let guard = ONE_AT_A_TIME.lock().unwrap_or_else(PoisonError::into_inner);
    let manifest = Manifest::builtin();
    let model = manifest.get("whisper-base").unwrap();
    let path = model.path(&repo().join("models"));
    assert!(
        path.is_file(),
        "model missing: {} (see this file's docs)",
        path.display()
    );
    (
        guard,
        WhisperAsr::load(&path, model.languages.clone()).unwrap(),
    )
}

/// How many reference lines of a clip the tests use. The whole clips are
/// 25–30 s; two lines (about 10 s) prove the same at a fraction of the time,
/// since decoding time grows with the text and en-dialogue would otherwise
/// need a second 30 s window.
const LINES: usize = 2;

/// Transcribes the first [`LINES`] lines of a clip and returns the WER against
/// its reference JSON. The audio is cut halfway into the silence after the
/// last line, so no word is split.
fn clip_wer(engine: &mut WhisperAsr, name: &str, language: Option<&str>) -> f64 {
    let base = repo().join("testdata/tts").join(name);
    let mut audio = saywut_capture_file::read_wav(&base.with_extension("wav")).unwrap();
    let mut reference =
        Transcript::from_json(&std::fs::read_to_string(base.with_extension("json")).unwrap())
            .unwrap();
    reference.segments.truncate(LINES);
    let cut_ms = reference.segments[LINES - 1].end_ms + 300;
    audio
        .samples
        .truncate((cut_ms * u64::from(audio.sample_rate) / 1000) as usize);
    let segments = engine.transcribe(&audio, language).unwrap();
    assert!(!segments.is_empty());
    assert!(segments.windows(2).all(|w| w[0].start_ms <= w[1].start_ms));
    assert!(
        segments
            .iter()
            .all(|s| s.end_ms <= audio.duration_ms() + 1000)
    );
    let join = |it: Vec<&str>| it.join(" ");
    let r = join(reference.segments.iter().map(|s| s.text.as_str()).collect());
    let h = join(segments.iter().map(|s| s.text.as_str()).collect());
    let score = wer(&r, &h);
    eprintln!("{name}: WER {:.1}%\n  ref: {r}\n  hyp: {h}", score * 100.0);
    score
}

#[test]
fn english_dialogue() {
    let (_guard, mut e) = engine();
    assert!(clip_wer(&mut e, "en-dialogue", Some("en")) < 0.15);
}

#[test]
fn dutch_monologue_with_region_tag_and_auto_detect() {
    let (_guard, mut e) = engine();
    // whisper-base gets 4 of the first 16 Dutch words wrong ("Goeden Morgen",
    // "spraakherkening"); the bound guards against regressions, the spike
    // measures quality.
    assert!(clip_wer(&mut e, "nl-monologue", Some("nl-NL")) < 0.30);
    assert!(clip_wer(&mut e, "nl-monologue", None) < 0.30);
}

#[test]
fn rejects_wrong_rate_and_unoffered_language() {
    let (_guard, mut e) = engine();
    assert_eq!(e.languages(), vec!["nl", "en", "de"]);
    let audio = Audio {
        sample_rate: 44_100,
        samples: vec![0.0; 441],
    };
    assert!(
        matches!(e.transcribe(&audio, None), Err(AsrError::Engine(m)) if m.contains("16000 Hz"))
    );
    let audio = Audio {
        sample_rate: 16_000,
        samples: vec![0.0; 16_000],
    };
    assert_eq!(
        e.transcribe(&audio, Some("fr")),
        Err(AsrError::UnsupportedLanguage("fr".into()))
    );
}
