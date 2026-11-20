//! `saywut speakers list|rename|merge <transcript.json>`.
//!
//! Speakers are addressed by the number in their default label ("Speaker 2"
//! is 2), which is their id plus one. Edits rewrite the file through a
//! temporary file in the same folder, so an interrupted write never leaves
//! half a transcript.

use std::{fs, io::Write, path::Path};

use clap::Subcommand;
use saywut_core::transcript::{EditError, SpeakerId, Transcript};
use saywut_i18n::Localizer;

use super::{path_value, speaker_label, write_result};

/// Speaker numbers as shown to the user start at 1.
fn number() -> clap::builder::RangedI64ValueParser<u32> {
    clap::value_parser!(u32).range(1..)
}

#[derive(Subcommand)]
pub(crate) enum SpeakersCommand {
    /// Show the speakers with their number, name and number of lines.
    List {
        /// Transcript JSON.
        file: std::path::PathBuf,
    },
    /// Name a speaker; an empty name removes it.
    Rename {
        /// Transcript JSON.
        file: std::path::PathBuf,
        /// Speaker number, as in "Speaker 2".
        #[arg(value_parser = number())]
        speaker: u32,
        /// The new name.
        name: String,
    },
    /// Give all lines of one speaker to another, for one person split in two.
    Merge {
        /// Transcript JSON.
        file: std::path::PathBuf,
        /// Speaker number whose lines move.
        #[arg(value_parser = number())]
        from: u32,
        /// Speaker number that gets them.
        #[arg(value_parser = number())]
        into: u32,
    },
}

fn id(number: u32) -> SpeakerId {
    SpeakerId(number - 1)
}

fn load(path: &Path, loc: &Localizer) -> Result<Transcript, String> {
    fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|json| Transcript::from_json(&json).map_err(|e| e.to_string()))
        .map_err(|reason| {
            loc.message(
                "error-read-transcript",
                &[("path", path_value(path)), ("reason", reason.into())],
            )
        })
}

fn save(path: &Path, transcript: &Transcript, loc: &Localizer) -> Result<(), String> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let result = fs::write(&tmp, transcript.to_json() + "\n").and_then(|()| fs::rename(&tmp, path));
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    write_result(result, path, loc)
}

fn edit_message(loc: &Localizer, error: &EditError) -> String {
    match error {
        EditError::UnknownSpeaker(id) => {
            loc.message("error-speaker-unknown", &[("n", (id.0 + 1).into())])
        }
        EditError::SameSpeaker(id) => {
            loc.message("error-speaker-same", &[("n", (id.0 + 1).into())])
        }
    }
}

pub(crate) fn run(
    cmd: SpeakersCommand,
    loc: &Localizer,
    out: &mut dyn Write,
) -> Result<(), String> {
    match cmd {
        SpeakersCommand::List { file } => {
            let t = load(&file, loc)?;
            let mut text = String::new();
            for speaker in &t.speakers {
                let lines = t
                    .segments
                    .iter()
                    .filter(|s| s.speaker == Some(speaker.id))
                    .count();
                let label = match &speaker.name {
                    Some(name) => name.clone(),
                    None => speaker_label(loc, speaker.id),
                };
                text.push_str(&loc.message(
                    "speakers-list-line",
                    &[
                        ("n", (speaker.id.0 + 1).into()),
                        ("label", label.into()),
                        ("lines", lines.into()),
                    ],
                ));
                text.push('\n');
            }
            write_result(out.write_all(text.as_bytes()), Path::new("-"), loc)
        }
        SpeakersCommand::Rename {
            file,
            speaker,
            name,
        } => {
            let mut t = load(&file, loc)?;
            t.rename_speaker(id(speaker), &name)
                .map_err(|e| edit_message(loc, &e))?;
            save(&file, &t, loc)
        }
        SpeakersCommand::Merge { file, from, into } => {
            let mut t = load(&file, loc)?;
            t.merge_speakers(id(from), id(into))
                .map_err(|e| edit_message(loc, &e))?;
            save(&file, &t, loc)
        }
    }
}
