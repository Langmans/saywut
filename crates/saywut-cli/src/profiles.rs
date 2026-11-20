//! `saywut profiles list|add|delete`: the local voice profiles.
//!
//! Profiles live in one JSON file, by default `voice-profiles.json` in the
//! user's local data folder. They hold voice embeddings, not audio, but are
//! biometric data all the same: they are only created by `add`, never
//! uploaded, and `delete` removes a person completely.

use std::{fs, io::Write, path::Path};

use clap::Subcommand;
use saywut_core::profiles::{VoiceProfiles, speaker_audio};
use saywut_i18n::Localizer;

use super::{EMBEDDING_MODEL, path_value, usable_model, write_result};
use crate::speakers::{id, load as load_transcript, number};

#[derive(Subcommand)]
pub(crate) enum ProfilesCommand {
    /// Show the stored voice profiles.
    List,
    /// Learn a voice: the speech of one speaker in a transcribed recording.
    /// Adding the same name again averages the recordings.
    Add {
        /// Name to store the voice under.
        name: String,
        /// The recording (WAV, 16 kHz).
        audio: std::path::PathBuf,
        /// Its transcript JSON, with speakers (from `transcribe --diarize`).
        transcript: std::path::PathBuf,
        /// Speaker number in that transcript, as in "Speaker 2".
        #[arg(value_parser = number())]
        speaker: u32,
    },
    /// Delete the voice profile of a name.
    Delete {
        /// Name of the profile.
        name: String,
    },
}

/// Reads the profiles; a file that does not exist yet means none.
pub(crate) fn load(path: &Path, loc: &Localizer) -> Result<VoiceProfiles, String> {
    if !path.exists() {
        return Ok(VoiceProfiles::default());
    }
    fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|json| VoiceProfiles::from_json(&json).map_err(|e| e.to_string()))
        .map_err(|reason| {
            loc.message(
                "error-read-profiles",
                &[("path", path_value(path)), ("reason", reason.into())],
            )
        })
}

fn save(path: &Path, profiles: &VoiceProfiles, loc: &Localizer) -> Result<(), String> {
    let result = path
        .parent()
        .map_or(Ok(()), fs::create_dir_all)
        .and_then(|()| fs::write(path, profiles.to_json() + "\n"));
    write_result(result, path, loc)
}

pub(crate) fn run(
    cmd: ProfilesCommand,
    store: &Path,
    model_dir: &Path,
    loc: &Localizer,
    out: &mut dyn Write,
) -> Result<(), String> {
    match cmd {
        ProfilesCommand::List => {
            let profiles = load(store, loc)?;
            let mut text = String::new();
            if profiles.profiles.is_empty() {
                text.push_str(&loc.message("profiles-none", &[]));
                text.push('\n');
            }
            for p in &profiles.profiles {
                text.push_str(&loc.message(
                    "profiles-list-line",
                    &[
                        ("name", p.name.clone().into()),
                        ("recordings", p.recordings.into()),
                    ],
                ));
                text.push('\n');
            }
            write_result(out.write_all(text.as_bytes()), Path::new("-"), loc)
        }
        ProfilesCommand::Add {
            name,
            audio,
            transcript,
            speaker,
        } => {
            let t = load_transcript(&transcript, loc)?;
            let samples = saywut_capture_file::read_wav(&audio).map_err(|e| {
                loc.message(
                    "error-read-audio",
                    &[
                        ("path", path_value(&audio)),
                        ("reason", e.to_string().into()),
                    ],
                )
            })?;
            let speech = speaker_audio(&samples, &t, id(speaker));
            if speech.samples.is_empty() {
                return Err(loc.message("error-speaker-silent", &[("n", speaker.into())]));
            }
            let (_, model) = usable_model(EMBEDDING_MODEL, loc, model_dir)?;
            let embedding = saywut_diarize_sherpa::SherpaEmbedder::load(&model)
                .and_then(|mut e| saywut_core::profiles::SpeakerEmbedder::embed(&mut e, &speech))
                .map_err(|e| loc.message("error-diarize", &[("reason", e.0.into())]))?;
            let mut profiles = load(store, loc)?;
            profiles.remember(&name, EMBEDDING_MODEL, &embedding);
            save(store, &profiles, loc)?;
            let line = loc.message("profiles-added", &[("name", name.into())]);
            write_result(writeln!(out, "{line}"), Path::new("-"), loc)
        }
        ProfilesCommand::Delete { name } => {
            let mut profiles = load(store, loc)?;
            if !profiles.forget(&name) {
                return Err(loc.message("error-profile-unknown", &[("name", name.into())]));
            }
            save(store, &profiles, loc)?;
            let line = loc.message("profiles-deleted", &[("name", name.into())]);
            write_result(writeln!(out, "{line}"), Path::new("-"), loc)
        }
    }
}
