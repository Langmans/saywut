//! The `saywut` command line: `transcribe`, `models`, `speakers` and
//! `profiles`; usage is in the repository README ("Using the CLI"). Messages
//! are localized; clap's own help and usage text is English.

mod profiles;
mod speakers;

use std::{
    ffi::OsString,
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use saywut_asr_whisper::WhisperAsr;
use saywut_core::{
    asr::{AsrEngine, AsrError, Audio, FakeAsr},
    diarize::{DiarizeError, Diarizer},
    export,
    pipeline::{apply_diarization, transcribe_file},
    profiles::{MATCH_THRESHOLD, VoiceProfiles, recognize},
    transcript::{SpeakerId, Transcript},
};
use saywut_diarize_sherpa::{SherpaDiarizer, SherpaEmbedder};
use saywut_i18n::{FALLBACK, FluentValue, Localizer};
use saywut_models::{Manifest, Model, ModelError, Opener};

/// Languages the fake engine accepts when its script names none.
const FAKE_LANGUAGES: [&str; 3] = ["en", "nl", "de"];

/// Manifest ids of the diarization models.
const SEGMENTATION_MODEL: &str = "pyannote-segmentation-3";
const EMBEDDING_MODEL: &str = "campplus-voxceleb";

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

    /// Where model files are stored. Defaults to the user's local data
    /// directory.
    #[arg(long, global = true, env = "SAYWUT_MODEL_DIR")]
    model_dir: Option<PathBuf>,

    /// The voice profiles file. Defaults to voice-profiles.json in the
    /// user's local data directory.
    #[arg(long, global = true, env = "SAYWUT_PROFILES")]
    profiles_file: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Transcribe an audio file (WAV, 16 kHz for whisper).
    Transcribe(TranscribeArgs),
    /// List or download speech models.
    #[command(subcommand)]
    Models(ModelsCommand),
    /// Show, name or merge the speakers of a transcript JSON (edited in place).
    #[command(subcommand)]
    Speakers(speakers::SpeakersCommand),
    /// Manage the local voice profiles (voice embeddings, no audio).
    #[command(subcommand)]
    Profiles(profiles::ProfilesCommand),
}

#[derive(Subcommand)]
enum ModelsCommand {
    /// Show the known models and whether they are downloaded.
    List,
    /// Download a model and verify its size and sha256.
    Fetch {
        /// Model id, as shown by `saywut models list`.
        id: String,
    },
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

    /// Speech recognition engine.
    #[arg(long, value_enum, default_value_t = Engine::Whisper)]
    engine: Engine,

    /// Model id for the engine, as shown by `saywut models list`.
    #[arg(long, default_value = "whisper-base")]
    model: String,

    /// Transcript JSON whose segments the fake engine plays back.
    #[arg(long)]
    fake_script: Option<PathBuf>,

    /// Label who speaks when (needs the diarization models, see
    /// `saywut models list`).
    #[arg(long)]
    diarize: bool,

    /// Number of speakers, if known; otherwise they are found automatically.
    #[arg(long, requires = "diarize", value_parser = clap::value_parser!(u32).range(1..))]
    speakers: Option<u32>,

    /// Name speakers whose voice matches a stored voice profile.
    #[arg(long, requires = "diarize")]
    recognize: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Json,
    Srt,
    Vtt,
    Txt,
}

#[derive(Clone, Copy, ValueEnum)]
enum Engine {
    Whisper,
    Fake,
}

/// Parses `args` (program name first) and runs the command. Returns the exit
/// code: 0 on success, 1 on a failed command, clap's code on a usage error.
pub fn run<I, T>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> u8
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    run_with(args, out, err, &saywut_models::http_get)
}

/// [`run`] with the function that opens download URLs passed in.
fn run_with<I, T>(args: I, out: &mut dyn Write, err: &mut dyn Write, open: Opener) -> u8
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
    let model_dir = cli.model_dir.unwrap_or_else(default_model_dir);
    let store = cli.profiles_file.unwrap_or_else(default_profiles_file);
    let result = match cli.command {
        Command::Transcribe(args) => transcribe(&args, &loc, &model_dir, &store, out),
        Command::Models(ModelsCommand::List) => list_models(&loc, &model_dir, out),
        Command::Models(ModelsCommand::Fetch { id }) => {
            fetch_model(&id, &loc, &model_dir, open, out)
        }
        Command::Speakers(cmd) => speakers::run(cmd, &loc, out),
        Command::Profiles(cmd) => profiles::run(cmd, &store, &model_dir, &loc, out),
    };
    match result {
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

/// Saywut's folder in the user's local data directory (the working
/// directory if the system has none).
fn data_dir() -> PathBuf {
    dirs::data_local_dir().map_or(PathBuf::new(), |d| d.join("saywut"))
}

fn default_model_dir() -> PathBuf {
    data_dir().join("models")
}

fn default_profiles_file() -> PathBuf {
    data_dir().join("voice-profiles.json")
}

/// "Speaker 1" etc. for a speaker without a name; ids count from 0, labels
/// from 1.
fn speaker_label(loc: &Localizer, id: SpeakerId) -> String {
    loc.message("speaker-label", &[("n", (u64::from(id.0) + 1).into())])
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

fn find_model<'m>(manifest: &'m Manifest, id: &str, loc: &Localizer) -> Result<&'m Model, String> {
    manifest
        .get(id)
        .map_err(|_| loc.message("error-model-unknown", &[("id", id.to_owned().into())]))
}

/// The manifest entry and file of a model that is downloaded and intact.
/// The file is re-hashed on every load: a damaged file must not reach an
/// engine.
fn usable_model(id: &str, loc: &Localizer, model_dir: &Path) -> Result<(Model, PathBuf), String> {
    let manifest = Manifest::builtin();
    let model = find_model(&manifest, id, loc)?.clone();
    let path = model.path(model_dir);
    let id: FluentValue = model.id.clone().into();
    if !path.is_file() {
        return Err(loc.message("error-model-missing", &[("id", id)]));
    }
    saywut_models::verify(&model, &path).map_err(|e| {
        loc.message(
            "error-model-corrupt",
            &[
                ("id", id.clone()),
                ("path", path_value(&path)),
                ("reason", e.to_string().into()),
            ],
        )
    })?;
    Ok((model, path))
}

/// The engine chosen on the command line. The outer error is a finished,
/// localized message (unknown, missing or damaged model, unreadable script);
/// the inner one is the engine's own and is localized together with the
/// engine's transcription errors.
fn engine(
    args: &TranscribeArgs,
    loc: &Localizer,
    model_dir: &Path,
) -> Result<Result<Box<dyn AsrEngine>, AsrError>, String> {
    match args.engine {
        Engine::Fake => {
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
            Ok(Ok(Box::new(FakeAsr {
                languages,
                script: script.segments,
            })))
        }
        Engine::Whisper => {
            let (model, path) = usable_model(&args.model, loc, model_dir)?;
            Ok(WhisperAsr::load(&path, model.languages).map(|e| Box::new(e) as Box<dyn AsrEngine>))
        }
    }
}

fn transcribe(
    args: &TranscribeArgs,
    loc: &Localizer,
    model_dir: &Path,
    profiles_file: &Path,
    out: &mut dyn Write,
) -> Result<(), String> {
    let audio = saywut_capture_file::read_wav(&args.input).map_err(|e| {
        loc.message(
            "error-read-audio",
            &[
                ("path", path_value(&args.input)),
                ("reason", e.to_string().into()),
            ],
        )
    })?;
    let mut transcript = engine(args, loc, model_dir)?
        .and_then(|mut engine| transcribe_file(engine.as_mut(), &audio, args.language.as_deref()))
        .map_err(|e| asr_message(loc, &e))?;

    if args.diarize {
        let (_, segmentation) = usable_model(SEGMENTATION_MODEL, loc, model_dir)?;
        let (_, embedding) = usable_model(EMBEDDING_MODEL, loc, model_dir)?;
        let profiles = if args.recognize {
            Some(profiles::load(profiles_file, loc)?)
        } else {
            None
        };
        label_speakers(
            &mut transcript,
            &audio,
            args.speakers,
            (&segmentation, &embedding),
            profiles.as_ref(),
        )
        .map_err(|e| loc.message("error-diarize", &[("reason", e.0.into())]))?;
    }

    let fallback = |id: SpeakerId| speaker_label(loc, id);
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
    write_result(result, target, loc)
}

/// Diarizes `audio` into `transcript` and, given profiles, names the voices
/// it knows.
fn label_speakers(
    transcript: &mut Transcript,
    audio: &Audio,
    speakers: Option<u32>,
    (segmentation, embedding): (&Path, &Path),
    profiles: Option<&VoiceProfiles>,
) -> Result<(), DiarizeError> {
    let turns = SherpaDiarizer::load(segmentation, embedding, speakers)?.diarize(audio)?;
    apply_diarization(transcript, turns);
    if let Some(profiles) = profiles {
        let mut embedder = SherpaEmbedder::load(embedding)?;
        recognize(
            transcript,
            audio,
            &mut embedder,
            profiles,
            EMBEDDING_MODEL,
            MATCH_THRESHOLD,
        )?;
    }
    Ok(())
}

fn write_result(result: std::io::Result<()>, target: &Path, loc: &Localizer) -> Result<(), String> {
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

fn list_models(loc: &Localizer, model_dir: &Path, out: &mut dyn Write) -> Result<(), String> {
    let mut text = String::new();
    for model in Manifest::builtin().models {
        let status = if model.path(model_dir).is_file() {
            "models-status-present"
        } else {
            "models-status-missing"
        };
        let languages = if model.languages.is_empty() {
            loc.message("models-languages-any", &[])
        } else {
            model.languages.join(", ")
        };
        let line = loc.message(
            "models-list-line",
            &[
                ("id", model.id.clone().into()),
                ("engine", model.engine.clone().into()),
                ("size", format!("{:.0}", model.size as f64 / 1e6).into()),
                ("languages", languages.into()),
                ("status", loc.message(status, &[]).into()),
            ],
        );
        text.push_str(&line);
        text.push('\n');
    }
    write_result(out.write_all(text.as_bytes()), Path::new("-"), loc)
}

fn fetch_model(
    id: &str,
    loc: &Localizer,
    model_dir: &Path,
    open: Opener,
    out: &mut dyn Write,
) -> Result<(), String> {
    let manifest = Manifest::builtin();
    let model = find_model(&manifest, id, loc)?;
    let path = saywut_models::fetch(model, model_dir, open).map_err(|e: ModelError| {
        loc.message(
            "error-model-fetch",
            &[
                ("id", id.to_owned().into()),
                ("reason", e.to_string().into()),
            ],
        )
    })?;
    let line = loc.message(
        "models-ready",
        &[("id", id.to_owned().into()), ("path", path_value(&path))],
    );
    write_result(writeln!(out, "{line}"), Path::new("-"), loc)
}

fn read_script(path: &Path) -> Result<Transcript, String> {
    let json = fs::read_to_string(path).map_err(|e| e.to_string())?;
    Transcript::from_json(&json).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests;
