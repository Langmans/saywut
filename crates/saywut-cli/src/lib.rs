//! `saywut transcribe <file>`: an audio file in, a transcript out.
//!
//! The only engine so far is the fake one, which replays the segments of a
//! transcript JSON given with `--fake-script`. That is enough to run the whole
//! chain (file reading, pipeline, export, localization) end to end; real
//! engines are added as further `--engine` values.
//!
//! Messages are localized; clap's own help and usage text is English.

use std::{
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use saywut_core::{
    asr::{AsrError, FakeAsr},
    export,
    pipeline::transcribe_file,
    transcript::{SpeakerId, Transcript},
};
use saywut_i18n::{FALLBACK, FluentValue, Localizer};

/// Languages the fake engine accepts when its script names none.
const FAKE_LANGUAGES: [&str; 3] = ["en", "nl", "de"];

#[derive(Parser)]
#[command(
    name = "saywut",
    version,
    about = "Transcribe speech with speaker labels"
)]
struct Cli {
    /// UI language for messages and speaker labels (BCP-47, e.g. nl, de-CH).
    /// Defaults to the system language.
    #[arg(long, global = true)]
    locale: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Transcribe an audio file (WAV).
    Transcribe(TranscribeArgs),
}

#[derive(Args)]
struct TranscribeArgs {
    /// Audio file to transcribe.
    input: PathBuf,

    /// Output format.
    #[arg(short, long, value_enum, default_value_t = Format::Json)]
    format: Format,

    /// Spoken language (BCP-47). Detected automatically when left out.
    #[arg(short, long)]
    language: Option<String>,

    /// Write to this file instead of standard output.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Transcript JSON whose segments the fake engine plays back.
    #[arg(long)]
    fake_script: Option<PathBuf>,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Json,
    Srt,
    Vtt,
    Txt,
}

/// Parses `args` (program name first) and runs the command. Returns the exit
/// code: 0 on success, 1 on a failed command, clap's code on a usage error.
pub fn run<I, T>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            let target: &mut dyn Write = if e.use_stderr() { err } else { out };
            let _ = write!(target, "{}", e.render());
            return e.exit_code() as u8;
        }
    };
    let locale = cli.locale.unwrap_or_else(system_locale);
    let loc = Localizer::new(&locale);
    let Command::Transcribe(args) = cli.command;
    match transcribe(&args, &loc, out) {
        Ok(()) => 0,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            1
        }
    }
}

fn system_locale() -> String {
    sys_locale::get_locale().unwrap_or(FALLBACK.to_owned())
}

fn path_value(path: &Path) -> FluentValue<'static> {
    path.display().to_string().into()
}

fn asr_message(loc: &Localizer, error: &AsrError) -> String {
    match error {
        AsrError::UnsupportedLanguage(tag) => loc.message(
            "error-unsupported-language",
            &[("language", tag.clone().into())],
        ),
        AsrError::Engine(reason) => {
            loc.message("error-asr-engine", &[("reason", reason.clone().into())])
        }
    }
}

fn transcribe(args: &TranscribeArgs, loc: &Localizer, out: &mut dyn Write) -> Result<(), String> {
    let audio = saywut_capture_file::read_wav(&args.input).map_err(|e| {
        loc.message(
            "error-read-audio",
            &[
                ("path", path_value(&args.input)),
                ("reason", e.to_string().into()),
            ],
        )
    })?;

    let script = match &args.fake_script {
        Some(path) => read_script(path).map_err(|reason| {
            loc.message(
                "error-read-script",
                &[("path", path_value(path)), ("reason", reason.into())],
            )
        })?,
        None => Transcript::default(),
    };
    let languages = match script.language {
        Some(tag) => vec![tag],
        None => FAKE_LANGUAGES.map(str::to_owned).to_vec(),
    };
    let mut engine = FakeAsr {
        languages,
        script: script.segments,
    };

    let transcript = transcribe_file(&mut engine, &audio, args.language.as_deref())
        .map_err(|e| asr_message(loc, &e))?;

    let fallback =
        |id: SpeakerId| loc.message("speaker-label", &[("n", (u64::from(id.0) + 1).into())]);
    let text = match args.format {
        Format::Json => transcript.to_json() + "\n",
        Format::Srt => export::to_srt(&transcript, &fallback),
        Format::Vtt => export::to_vtt(&transcript, &fallback),
        Format::Txt => export::to_txt(&transcript, &fallback),
    };

    let (result, target) = match &args.output {
        Some(path) => (fs::write(path, &text), path.as_path()),
        None => (out.write_all(text.as_bytes()), Path::new("-")),
    };
    result.map_err(|e| {
        loc.message(
            "error-write-output",
            &[
                ("path", path_value(target)),
                ("reason", e.to_string().into()),
            ],
        )
    })
}

fn read_script(path: &Path) -> Result<Transcript, String> {
    let json = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Transcript::from_json(&json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{SampleFormat, WavSpec, WavWriter};

    /// A scratch directory per test, removed again on drop.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("saywut-cli-{}-{name}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self, file: &str) -> PathBuf {
            self.0.join(file)
        }

        /// Two seconds of silence at 16 kHz.
        fn wav(&self) -> PathBuf {
            let path = self.path("in.wav");
            let spec = WavSpec {
                channels: 1,
                sample_rate: 16_000,
                bits_per_sample: 16,
                sample_format: SampleFormat::Int,
            };
            let mut w = WavWriter::create(&path, spec).unwrap();
            for _ in 0..32_000 {
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

    fn saywut(args: &[&str]) -> Run {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(
            std::iter::once("saywut").chain(args.iter().copied()),
            &mut out,
            &mut err,
        );
        Run {
            code,
            out: String::from_utf8(out).unwrap(),
            err: String::from_utf8(err).unwrap(),
        }
    }

    fn s(p: &Path) -> &str {
        p.to_str().unwrap()
    }

    #[test]
    fn help_goes_to_stdout() {
        let r = saywut(&["--help"]);
        assert_eq!(r.code, 0);
        assert!(r.out.contains("transcribe"));
    }

    #[test]
    fn usage_error_goes_to_stderr() {
        let r = saywut(&["transcribe"]);
        assert_eq!(r.code, 2);
        assert!(r.err.contains("<INPUT>"));
    }

    #[test]
    fn srt_with_localized_speaker_labels() {
        let dir = Scratch::new("srt");
        let (wav, script) = (dir.wav(), dir.script(SCRIPT));
        let r = saywut(&[
            "--locale",
            "nl",
            "transcribe",
            s(&wav),
            "-f",
            "srt",
            "--fake-script",
            s(&script),
        ]);
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
        let vtt = saywut(&[
            "--locale",
            "de",
            "transcribe",
            s(&wav),
            "-f",
            "vtt",
            "--fake-script",
            s(&script),
        ]);
        assert!(vtt.out.starts_with("WEBVTT\n\n"));
        assert!(vtt.out.contains("<v Sprecher 2>Hoi."));
        let txt = saywut(&[
            "--locale",
            "en",
            "transcribe",
            s(&wav),
            "-f",
            "txt",
            "--fake-script",
            s(&script),
        ]);
        assert_eq!(
            txt.out,
            "[00:00:00] Speaker 1: Hallo.\n[00:00:01] Speaker 2: Hoi.\n"
        );
    }

    #[test]
    fn json_is_the_default_and_carries_the_language() {
        let dir = Scratch::new("json");
        let (wav, script) = (dir.wav(), dir.script(SCRIPT));
        let r = saywut(&[
            "transcribe",
            s(&wav),
            "-l",
            "nl",
            "--fake-script",
            s(&script),
        ]);
        assert_eq!(r.code, 0, "{}", r.err);
        let t = Transcript::from_json(&r.out).unwrap();
        assert_eq!(t.language.as_deref(), Some("nl"));
        assert_eq!(t.speakers.len(), 2);
        assert_eq!(t.segments.len(), 2);
    }

    #[test]
    fn without_script_the_transcript_is_empty() {
        let dir = Scratch::new("empty");
        let r = saywut(&["transcribe", s(&dir.wav()), "-f", "srt"]);
        assert_eq!((r.code, r.out.as_str()), (0, ""));
    }

    #[test]
    fn writes_to_output_file() {
        let dir = Scratch::new("output");
        let (wav, script, srt) = (dir.wav(), dir.script(SCRIPT), dir.path("out.srt"));
        let r = saywut(&[
            "transcribe",
            s(&wav),
            "-f",
            "srt",
            "-o",
            s(&srt),
            "--fake-script",
            s(&script),
        ]);
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
        let missing = saywut(&[
            "--locale",
            "en",
            "transcribe",
            s(&wav),
            "--fake-script",
            s(&dir.path("none.json")),
        ]);
        assert_eq!(missing.code, 1);
        assert!(
            missing.err.starts_with("Could not read fake script"),
            "{}",
            missing.err
        );

        let invalid = dir.script("{");
        let r = saywut(&[
            "--locale",
            "en",
            "transcribe",
            s(&wav),
            "--fake-script",
            s(&invalid),
        ]);
        assert_eq!(r.code, 1);
        assert!(r.err.contains("invalid transcript JSON"), "{}", r.err);
    }

    #[test]
    fn script_language_limits_the_fake_engine() {
        let dir = Scratch::new("language");
        let wav = dir.wav();
        let script = dir.script(r#"{"version": 1, "language": "en"}"#);
        let ok = saywut(&[
            "transcribe",
            s(&wav),
            "-l",
            "en",
            "--fake-script",
            s(&script),
        ]);
        assert_eq!(ok.code, 0, "{}", ok.err);
        let r = saywut(&[
            "--locale",
            "nl",
            "transcribe",
            s(&wav),
            "-l",
            "de",
            "--fake-script",
            s(&script),
        ]);
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
        let wav = dir.wav();
        let target = dir.path("no-such-dir").join("out.json");
        let r = saywut(&["--locale", "en", "transcribe", s(&wav), "-o", s(&target)]);
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
        let code = run(
            ["saywut", "--locale", "en", "transcribe", s(&wav)],
            &mut Broken,
            &mut err,
        );
        assert_eq!(code, 1);
        assert_eq!(
            String::from_utf8(err).unwrap(),
            "Could not write -: pipe closed\n"
        );
        assert!(Broken.flush().is_ok());
    }

    #[test]
    fn system_locale_is_used_when_none_is_given() {
        assert!(!system_locale().is_empty());
    }
}
