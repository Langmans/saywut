use super::*;
use hound::{SampleFormat, WavSpec, WavWriter};
use std::io::Read;

/// A scratch directory per test, removed again on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("saywut-cli-{}-{name}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }

    fn path(&self, file: &str) -> PathBuf {
        self.0.join(file)
    }

    /// Two seconds of silence at 16 kHz.
    fn wav(&self) -> PathBuf {
        self.wav_at(16_000)
    }

    /// Two seconds of silence at `rate`.
    fn wav_at(&self, rate: u32) -> PathBuf {
        let path = self.path("in.wav");
        let spec = WavSpec {
            channels: 1,
            sample_rate: rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut w = WavWriter::create(&path, spec).unwrap();
        for _ in 0..rate * 2 {
            w.write_sample(0_i16).unwrap();
        }
        w.finalize().unwrap();
        path
    }

    fn script(&self, json: &str) -> PathBuf {
        let path = self.path("script.json");
        fs::write(&path, json).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// The repository root; its `models/` holds the real whisper-base for the
/// tests that need it (see crates/asr-whisper/tests/model.rs).
fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const SCRIPT: &str = r#"{
    "version": 1,
    "segments": [
        {"start_ms": 0, "end_ms": 900, "speaker": 0, "text": "Hallo."},
        {"start_ms": 1000, "end_ms": 1800, "speaker": 1, "text": "Hoi."}
    ]
}"#;

struct Run {
    code: u8,
    out: String,
    err: String,
}

/// Tests never touch the network; a download attempt fails visibly.
fn no_network(url: &str) -> Result<Box<dyn Read>, ModelError> {
    Err(ModelError::Download(format!("no network in tests ({url})")))
}

fn saywut_with(args: &[&str], open: Opener) -> Run {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = run_with(
        std::iter::once("saywut").chain(args.iter().copied()),
        &mut out,
        &mut err,
        open,
    );
    Run {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn saywut(args: &[&str]) -> Run {
    saywut_with(args, &no_network)
}

/// `saywut transcribe <wav> --engine fake [--fake-script <script>] <extra>`.
fn fake(wav: &Path, script: Option<&Path>, extra: &[&str]) -> Run {
    let mut args = vec!["transcribe", s(wav), "--engine", "fake"];
    if let Some(script) = script {
        args.extend(["--fake-script", s(script)]);
    }
    args.extend(extra);
    saywut(&args)
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn help_goes_to_stdout() {
    let r = saywut(&["--help"]);
    assert_eq!(r.code, 0);
    assert!(r.out.contains("transcribe") && r.out.contains("models"));
}

#[test]
fn usage_error_goes_to_stderr() {
    let r = saywut(&["transcribe"]);
    assert_eq!(r.code, 2);
    assert!(r.err.contains("<INPUT>"));
}

#[test]
fn run_uses_the_real_opener() {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(run(["saywut", "--version"], &mut out, &mut err), 0);
}

#[test]
fn srt_with_localized_speaker_labels() {
    let dir = Scratch::new("srt");
    let (wav, script) = (dir.wav(), dir.script(SCRIPT));
    let r = fake(&wav, Some(&script), &["-f", "srt", "--locale", "nl"]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert_eq!(
        r.out,
        "1\n00:00:00,000 --> 00:00:00,900\nSpreker 1: Hallo.\n\n\
         2\n00:00:01,000 --> 00:00:01,800\nSpreker 2: Hoi.\n\n"
    );
}

#[test]
fn vtt_and_txt() {
    let dir = Scratch::new("vtt-txt");
    let (wav, script) = (dir.wav(), dir.script(SCRIPT));
    let vtt = fake(&wav, Some(&script), &["-f", "vtt", "--locale", "de"]);
    assert!(vtt.out.starts_with("WEBVTT\n\n"));
    assert!(vtt.out.contains("<v Sprecher 2>Hoi."));
    let txt = fake(&wav, Some(&script), &["-f", "txt", "--locale", "en"]);
    assert_eq!(
        txt.out,
        "[00:00:00] Speaker 1: Hallo.\n[00:00:01] Speaker 2: Hoi.\n"
    );
}

#[test]
fn json_is_the_default_and_carries_the_language() {
    let dir = Scratch::new("json");
    let (wav, script) = (dir.wav(), dir.script(SCRIPT));
    let r = fake(&wav, Some(&script), &["-l", "nl"]);
    assert_eq!(r.code, 0, "{}", r.err);
    let t = Transcript::from_json(&r.out).unwrap();
    assert_eq!(t.language.as_deref(), Some("nl"));
    assert_eq!(t.speakers.len(), 2);
    assert_eq!(t.segments.len(), 2);
}

#[test]
fn without_script_the_transcript_is_empty() {
    let dir = Scratch::new("empty");
    let r = fake(&dir.wav(), None, &["-f", "srt"]);
    assert_eq!((r.code, r.out.as_str()), (0, ""));
}

#[test]
fn writes_to_output_file() {
    let dir = Scratch::new("output");
    let (wav, script, srt) = (dir.wav(), dir.script(SCRIPT), dir.path("out.srt"));
    let r = fake(&wav, Some(&script), &["-f", "srt", "-o", s(&srt)]);
    assert_eq!((r.code, r.out.as_str()), (0, ""));
    assert!(fs::read_to_string(&srt).unwrap().contains("Hallo."));
}

#[test]
fn missing_audio_is_a_localized_error() {
    let r = saywut(&["--locale", "nl", "transcribe", "geen-bestand.wav"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err
            .starts_with("Kon audiobestand geen-bestand.wav niet lezen: "),
        "{}",
        r.err
    );
}

#[test]
fn unreadable_or_invalid_script() {
    let dir = Scratch::new("bad-script");
    let wav = dir.wav();
    let missing = fake(&wav, Some(&dir.path("none.json")), &["--locale", "en"]);
    assert_eq!(missing.code, 1);
    assert!(
        missing.err.starts_with("Could not read fake script"),
        "{}",
        missing.err
    );
    let invalid = dir.script("{");
    let r = fake(&wav, Some(&invalid), &["--locale", "en"]);
    assert_eq!(r.code, 1);
    assert!(r.err.contains("invalid transcript JSON"), "{}", r.err);
}

#[test]
fn script_language_limits_the_fake_engine() {
    let dir = Scratch::new("language");
    let wav = dir.wav();
    let script = dir.script(r#"{"version": 1, "language": "en"}"#);
    let ok = fake(&wav, Some(&script), &["-l", "en"]);
    assert_eq!(ok.code, 0, "{}", ok.err);
    let r = fake(&wav, Some(&script), &["-l", "de", "--locale", "nl"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err, "Taal de wordt niet ondersteund door deze engine.\n");
}

#[test]
fn engine_failure_message() {
    let loc = Localizer::new("en");
    assert_eq!(
        asr_message(&loc, &AsrError::Engine("out of memory".into())),
        "Speech recognition failed: out of memory"
    );
}

#[test]
fn unwritable_output_file() {
    let dir = Scratch::new("unwritable");
    let target = dir.path("no-such-dir").join("out.json");
    let r = fake(&dir.wav(), None, &["--locale", "en", "-o", s(&target)]);
    assert_eq!(r.code, 1);
    assert!(r.err.starts_with("Could not write "), "{}", r.err);
}

#[test]
fn failing_stdout() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("pipe closed"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let dir = Scratch::new("broken-stdout");
    let wav = dir.wav();
    let mut err = Vec::new();
    let args = [
        "saywut",
        "--locale",
        "en",
        "transcribe",
        s(&wav),
        "--engine",
        "fake",
    ];
    let code = run_with(args, &mut Broken, &mut err, &no_network);
    assert_eq!(code, 1);
    assert_eq!(
        String::from_utf8(err).unwrap(),
        "Could not write -: pipe closed\n"
    );
    assert!(Broken.flush().is_ok());
}

#[test]
fn system_defaults() {
    assert!(!system_locale().is_empty());
    assert!(default_model_dir().ends_with(Path::new("saywut").join("models")));
}

// --- whisper and models ---

/// Whisper and diarization together on the two-voice clip: the lines
/// alternate between Speaker 1 and Speaker 2.
#[test]
fn whisper_transcribes_and_diarizes_a_clip() {
    let wav = repo().join("testdata/tts/en-dialogue.wav");
    let models = repo().join("models");
    let r = saywut(&[
        "--locale",
        "en",
        "--model-dir",
        s(&models),
        "transcribe",
        s(&wav),
        "-l",
        "en",
        "-f",
        "txt",
        "--diarize",
        "--speakers",
        "2",
    ]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(r.out.to_lowercase().contains("report"), "{}", r.out);
    // "[00:00:05] Speaker 2: Almost. ..." -> "Speaker 2"
    let speakers: Vec<&str> = r
        .out
        .lines()
        .filter_map(|l| l.split_once("] ")?.1.split_once(": "))
        .map(|(who, _)| who)
        .collect();
    assert_eq!(speakers[..2], ["Speaker 1", "Speaker 2"], "{}", r.out);
}

#[test]
fn diarize_errors() {
    let dir = Scratch::new("diarize-errors");
    // Models missing.
    let r = fake(
        &dir.wav(),
        None,
        &["--locale", "en", "--model-dir", s(&dir.0), "--diarize"],
    );
    assert_eq!(
        r.err,
        "Model pyannote-segmentation-3 is not downloaded yet. \
         Run: saywut models fetch pyannote-segmentation-3\n"
    );
    // Wrong sample rate: the fake engine accepts it, the diarizer does not.
    let models = repo().join("models");
    let r = fake(
        &dir.wav_at(8_000),
        None,
        &["--locale", "en", "--model-dir", s(&models), "--diarize"],
    );
    assert_eq!(r.code, 1);
    assert!(
        r.err.starts_with("Diarization failed: needs 16000 Hz"),
        "{}",
        r.err
    );
    // --speakers needs --diarize, and at least 1.
    assert_eq!(fake(&dir.wav(), None, &["--speakers", "2"]).code, 2);
    assert_eq!(
        fake(&dir.wav(), None, &["--diarize", "--speakers", "0"]).code,
        2
    );
}

// --- speakers ---

const TWO_SPEAKERS: &str = r#"{
    "version": 1,
    "speakers": [{"id": 0, "name": "Ruben"}, {"id": 1}],
    "segments": [
        {"start_ms": 0, "end_ms": 900, "speaker": 0, "text": "Hallo."},
        {"start_ms": 1000, "end_ms": 1800, "speaker": 1, "text": "Hoi."},
        {"start_ms": 2000, "end_ms": 2800, "speaker": 1, "text": "Alles goed?"}
    ]
}"#;

fn speakers_cmd(args: &[&str]) -> Run {
    saywut(&[&["--locale", "nl", "speakers"], args].concat())
}

#[test]
fn speakers_list_rename_merge() {
    let dir = Scratch::new("speakers");
    let file = dir.script(TWO_SPEAKERS);
    let f = s(&file);

    let r = speakers_cmd(&["list", f]);
    assert_eq!(r.out, "1  Ruben  1 regel\n2  Spreker 2  2 regels\n");

    assert_eq!(speakers_cmd(&["rename", f, "2", "Anna"]).code, 0);
    assert!(speakers_cmd(&["list", f]).out.contains("2  Anna  2 regels"));

    assert_eq!(speakers_cmd(&["merge", f, "1", "2"]).code, 0);
    assert_eq!(speakers_cmd(&["list", f]).out, "2  Anna  3 regels\n");
    let t = Transcript::from_json(&fs::read_to_string(&file).unwrap()).unwrap();
    assert!(t.segments.iter().all(|s| s.speaker == Some(SpeakerId(1))));
}

#[test]
fn speakers_errors() {
    let dir = Scratch::new("speakers-errors");
    let file = dir.script(TWO_SPEAKERS);
    let f = s(&file);

    let r = speakers_cmd(&["rename", f, "5", "X"]);
    assert_eq!(
        (r.code, r.err.as_str()),
        (1, "Er is geen spreker 5 in dit transcript.\n")
    );
    let r = speakers_cmd(&["merge", f, "2", "2"]);
    assert_eq!(
        r.err,
        "Spreker 2 kan niet met zichzelf worden samengevoegd.\n"
    );
    assert_eq!(speakers_cmd(&["rename", f, "0", "X"]).code, 2);

    let r = speakers_cmd(&["list", s(&dir.path("missing.json"))]);
    assert!(r.err.starts_with("Kon transcript "), "{}", r.err);
    let broken = dir.path("broken.json");
    fs::write(&broken, "{").unwrap();
    let r = speakers_cmd(&["list", s(&broken)]);
    assert!(r.err.contains("invalid transcript JSON"), "{}", r.err);

    // The temporary file cannot be written: the transcript stays as it was.
    let mut tmp = file.as_os_str().to_owned();
    tmp.push(".tmp");
    fs::create_dir(&tmp).unwrap();
    let r = speakers_cmd(&["rename", f, "2", "Anna"]);
    assert_eq!(r.code, 1);
    assert!(r.err.starts_with("Kon "), "{}", r.err);
    assert_eq!(fs::read_to_string(&file).unwrap(), TWO_SPEAKERS);
}

#[test]
fn whisper_model_missing_corrupt_or_unknown() {
    let dir = Scratch::new("whisper-errors");
    let wav = dir.wav();
    let models = dir.path("models");
    let base = [
        "--locale",
        "en",
        "--model-dir",
        s(&models),
        "transcribe",
        s(&wav),
    ];

    let r = saywut(&base);
    assert_eq!(
        r.err,
        "Model whisper-base is not downloaded yet. Run: saywut models fetch whisper-base\n"
    );

    let manifest = Manifest::builtin();
    let path = manifest.get("whisper-base").unwrap().path(&models);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"not a model").unwrap();
    let r = saywut(&base);
    assert!(r.err.starts_with("Model file "), "{}", r.err);
    assert!(r.err.contains("is damaged"), "{}", r.err);

    let r = saywut(&[&base[..], &["--model", "nope"]].concat());
    assert_eq!(r.code, 1);
    assert_eq!(r.err, "Unknown model nope. See: saywut models list\n");
}

#[test]
fn models_list_shows_status() {
    let dir = Scratch::new("list");
    let empty = saywut(&["--locale", "en", "--model-dir", s(&dir.0), "models", "list"]);
    assert_eq!(empty.code, 0);
    assert_eq!(
        empty.out,
        "whisper-base  whisper  148 MB  nl, en, de  not downloaded\n\
         pyannote-segmentation-3  diarization  6 MB  any language  not downloaded\n\
         campplus-voxceleb  speaker-embedding  30 MB  any language  not downloaded\n"
    );
    let models = repo().join("models");
    let present = saywut(&[
        "--locale",
        "nl",
        "--model-dir",
        s(&models),
        "models",
        "list",
    ]);
    assert!(
        present.out.ends_with("gedownload\n") && !present.out.contains("niet"),
        "{}",
        present.out
    );
}

#[test]
fn models_fetch() {
    // Already present and intact: nothing is downloaded.
    let models = repo().join("models");
    let r = saywut(&[
        "--locale",
        "en",
        "--model-dir",
        s(&models),
        "models",
        "fetch",
        "whisper-base",
    ]);
    assert_eq!(r.code, 0, "{}", r.err);
    assert!(
        r.out.starts_with("Model whisper-base is ready: "),
        "{}",
        r.out
    );

    let dir = Scratch::new("fetch");
    let short = |_: &str| -> Result<Box<dyn Read>, ModelError> { Ok(Box::new(&b"tiny"[..])) };
    let args = [
        "--locale",
        "en",
        "--model-dir",
        s(&dir.0),
        "models",
        "fetch",
        "whisper-base",
    ];
    let r = saywut_with(&args, &short);
    assert_eq!(r.code, 1);
    assert!(
        r.err
            .starts_with("Could not fetch model whisper-base: size is 4 bytes"),
        "{}",
        r.err
    );

    let r = saywut(&args);
    assert!(
        r.err.contains("download failed: no network in tests"),
        "{}",
        r.err
    );

    let r = saywut(&[
        "--locale",
        "en",
        "--model-dir",
        s(&dir.0),
        "models",
        "fetch",
        "nope",
    ]);
    assert_eq!(r.err, "Unknown model nope. See: saywut models list\n");
}
