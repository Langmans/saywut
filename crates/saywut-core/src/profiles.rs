//! Voice profiles: one voice embedding per named person, to recognize them in
//! later recordings by cosine similarity. No audio is stored, but embeddings
//! are biometric data: created only on request, local, deletable per name.
//! See the crate README.

use serde::{Deserialize, Serialize};

use crate::asr::Audio;
use crate::diarize::DiarizeError;
use crate::transcript::{SpeakerId, Transcript};

/// Version of the JSON layout written by [`VoiceProfiles::to_json`].
pub const FORMAT_VERSION: u32 = 1;

/// Cosine similarity from which two voices count as the same person.
pub const MATCH_THRESHOLD: f32 = 0.6;

/// Most speech used for one embedding; more adds time, not accuracy.
pub const MAX_SPEECH_MS: u64 = 30_000;

/// Turns speech into a voice embedding.
pub trait SpeakerEmbedder {
    /// The embedding of all of `audio`, which should be one voice.
    fn embed(&mut self, audio: &Audio) -> Result<Vec<f32>, DiarizeError>;
}

/// The stored voice of one person.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceProfile {
    /// The person's name, as the user gave it.
    pub name: String,
    /// Manifest id of the embedding model; embeddings of different models
    /// cannot be compared.
    pub model: String,
    /// Average of the unit-length embeddings of every recording added.
    pub embedding: Vec<f32>,
    /// Number of recordings averaged into `embedding`.
    pub recordings: u32,
}

/// All stored voices, as in the profiles JSON file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VoiceProfiles {
    /// Layout version, [`FORMAT_VERSION`] when written by this build.
    pub version: u32,
    /// One profile per name.
    #[serde(default)]
    pub profiles: Vec<VoiceProfile>,
}

impl Default for VoiceProfiles {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            profiles: Vec::new(),
        }
    }
}

/// Why a profiles file could not be read.
#[derive(Debug)]
pub enum ProfilesError {
    /// Not valid profiles JSON.
    Json(serde_json::Error),
    /// Written by a newer Saywut with a layout this build does not know.
    UnsupportedVersion(u32),
}

impl std::fmt::Display for ProfilesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Json(e) => write!(f, "invalid voice profiles JSON: {e}"),
            Self::UnsupportedVersion(v) => write!(
                f,
                "voice profiles version {v} is newer than supported version {FORMAT_VERSION}"
            ),
        }
    }
}

impl std::error::Error for ProfilesError {}

fn unit(v: &[f32]) -> Vec<f32> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm == 0.0 {
        return v.to_vec();
    }
    v.iter().map(|x| x / norm).collect()
}

/// Cosine similarity, 0.0 for vectors of different length or zero length.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let (a, b) = (unit(a), unit(b));
    a.iter().zip(&b).map(|(x, y)| x * y).sum()
}

impl VoiceProfiles {
    /// Pretty-printed JSON for the profiles file.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("profiles serialize")
    }

    /// Reads a profiles file, refusing layouts newer than this build knows.
    pub fn from_json(json: &str) -> Result<Self, ProfilesError> {
        let profiles: Self = serde_json::from_str(json).map_err(ProfilesError::Json)?;
        if profiles.version > FORMAT_VERSION {
            return Err(ProfilesError::UnsupportedVersion(profiles.version));
        }
        Ok(profiles)
    }

    /// Adds a recording of `name`. A known name from the same model gets the
    /// running average of its recordings; from another model the profile
    /// starts over, since old and new embeddings cannot be mixed.
    pub fn remember(&mut self, name: &str, model: &str, embedding: &[f32]) {
        let e = unit(embedding);
        match self.profiles.iter_mut().find(|p| p.name == name) {
            Some(p) if p.model == model && p.embedding.len() == e.len() => {
                let n = p.recordings as f32;
                for (avg, x) in p.embedding.iter_mut().zip(&e) {
                    *avg = (*avg * n + x) / (n + 1.0);
                }
                p.recordings += 1;
            }
            Some(p) => {
                *p = VoiceProfile {
                    name: name.to_owned(),
                    model: model.to_owned(),
                    embedding: e,
                    recordings: 1,
                };
            }
            None => self.profiles.push(VoiceProfile {
                name: name.to_owned(),
                model: model.to_owned(),
                embedding: e,
                recordings: 1,
            }),
        }
    }

    /// Deletes the profile of `name`; false when there was none.
    pub fn forget(&mut self, name: &str) -> bool {
        let before = self.profiles.len();
        self.profiles.retain(|p| p.name != name);
        self.profiles.len() != before
    }

    /// The most similar profile of `model` at or above `threshold`.
    pub fn best_match(
        &self,
        model: &str,
        embedding: &[f32],
        threshold: f32,
    ) -> Option<(&VoiceProfile, f32)> {
        self.profiles
            .iter()
            .filter(|p| p.model == model)
            .map(|p| (p, cosine(&p.embedding, embedding)))
            .filter(|(_, score)| *score >= threshold)
            .max_by(|a, b| a.1.total_cmp(&b.1))
    }
}

/// The speech of one speaker: their segments' samples back to back, at most
/// [`MAX_SPEECH_MS`].
pub fn speaker_audio(audio: &Audio, transcript: &Transcript, speaker: SpeakerId) -> Audio {
    let rate = u64::from(audio.sample_rate);
    let max = (MAX_SPEECH_MS * rate / 1000) as usize;
    let mut samples = Vec::new();
    for seg in transcript
        .segments
        .iter()
        .filter(|s| s.speaker == Some(speaker))
    {
        let start = ((seg.start_ms * rate / 1000) as usize).min(audio.samples.len());
        let end = ((seg.end_ms * rate / 1000) as usize).clamp(start, audio.samples.len());
        samples.extend_from_slice(&audio.samples[start..end]);
        if samples.len() >= max {
            samples.truncate(max);
            break;
        }
    }
    Audio {
        sample_rate: audio.sample_rate,
        samples,
    }
}

/// Names unnamed speakers whose voice matches a profile. Each profile names
/// at most one speaker: when two speakers match the same profile, the closer
/// one gets it. Returns the speakers named, with the name and similarity.
pub fn recognize(
    transcript: &mut Transcript,
    audio: &Audio,
    embedder: &mut dyn SpeakerEmbedder,
    profiles: &VoiceProfiles,
    model: &str,
    threshold: f32,
) -> Result<Vec<(SpeakerId, String, f32)>, DiarizeError> {
    let mut candidates: Vec<(SpeakerId, String, f32)> = Vec::new();
    let unnamed: Vec<SpeakerId> = transcript
        .speakers
        .iter()
        .filter(|s| s.name.is_none())
        .map(|s| s.id)
        .collect();
    for id in unnamed {
        let speech = speaker_audio(audio, transcript, id);
        if speech.samples.is_empty() {
            continue;
        }
        let embedding = embedder.embed(&speech)?;
        if let Some((profile, score)) = profiles.best_match(model, &embedding, threshold) {
            candidates.push((id, profile.name.clone(), score));
        }
    }
    candidates.sort_by(|a, b| b.2.total_cmp(&a.2));
    let mut named: Vec<(SpeakerId, String, f32)> = Vec::new();
    for (id, name, score) in candidates {
        if named.iter().all(|(_, n, _)| *n != name) {
            transcript
                .rename_speaker(id, &name)
                .expect("speaker taken from the transcript");
            named.push((id, name, score));
        }
    }
    named.sort_by_key(|(id, _, _)| *id);
    Ok(named)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::{Segment, Speaker};

    const MODEL: &str = "test-model";

    /// Embeds as the average of the audio: the tests choose the "voice" by
    /// the sample values.
    struct MeanEmbedder;

    impl SpeakerEmbedder for MeanEmbedder {
        fn embed(&mut self, audio: &Audio) -> Result<Vec<f32>, DiarizeError> {
            let n = audio.samples.len() as f32;
            let mean = audio.samples.iter().sum::<f32>() / n;
            Ok(vec![mean, 1.0 - mean.abs()])
        }
    }

    struct FailingEmbedder;

    impl SpeakerEmbedder for FailingEmbedder {
        fn embed(&mut self, _: &Audio) -> Result<Vec<f32>, DiarizeError> {
            Err(DiarizeError("model broke".into()))
        }
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn cosine_similarity() {
        assert!(approx(cosine(&[1.0, 0.0], &[2.0, 0.0]), 1.0));
        assert!(approx(cosine(&[1.0, 0.0], &[0.0, 3.0]), 0.0));
        assert!(approx(cosine(&[1.0, 0.0], &[-1.0, 0.0]), -1.0));
        assert_eq!(cosine(&[1.0], &[1.0, 0.0]), 0.0);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
    }

    #[test]
    fn remember_averages_and_restarts_on_model_change() {
        let mut p = VoiceProfiles::default();
        p.remember("Anna", MODEL, &[2.0, 0.0]);
        p.remember("Anna", MODEL, &[0.0, 5.0]);
        let anna = &p.profiles[0];
        assert_eq!(anna.recordings, 2);
        assert!(approx(anna.embedding[0], 0.5) && approx(anna.embedding[1], 0.5));

        p.remember("Anna", "other-model", &[0.0, 1.0, 0.0]);
        assert_eq!(p.profiles[0].recordings, 1);
        assert_eq!(p.profiles[0].model, "other-model");
        assert_eq!(p.profiles.len(), 1);
    }

    #[test]
    fn forget_and_json_round_trip() {
        let mut p = VoiceProfiles::default();
        p.remember("Anna", MODEL, &[1.0, 0.0]);
        p.remember("Bas", MODEL, &[0.0, 1.0]);
        assert_eq!(VoiceProfiles::from_json(&p.to_json()).unwrap(), p);
        assert!(p.forget("Anna"));
        assert!(!p.forget("Anna"));
        assert_eq!(p.profiles.len(), 1);
    }

    #[test]
    fn json_errors() {
        let err = VoiceProfiles::from_json("{").unwrap_err();
        assert!(err.to_string().starts_with("invalid voice profiles JSON"));
        let err = VoiceProfiles::from_json(r#"{"version": 9}"#).unwrap_err();
        assert!(matches!(err, ProfilesError::UnsupportedVersion(9)));
        assert!(err.to_string().contains("version 9"));
        assert_eq!(
            VoiceProfiles::from_json(r#"{"version": 1}"#).unwrap(),
            VoiceProfiles::default()
        );
    }

    #[test]
    fn best_match_respects_model_and_threshold() {
        let mut p = VoiceProfiles::default();
        p.remember("Anna", MODEL, &[1.0, 0.0]);
        p.remember("Bas", MODEL, &[0.6, 0.8]);
        p.remember("Cor", "other-model", &[1.0, 0.0]);
        let (who, score) = p.best_match(MODEL, &[0.9, 0.1], MATCH_THRESHOLD).unwrap();
        assert_eq!(who.name, "Anna");
        assert!(score > 0.9);
        assert!(p.best_match(MODEL, &[-1.0, 0.0], MATCH_THRESHOLD).is_none());
        assert!(p.best_match("unknown-model", &[1.0, 0.0], 0.0).is_none());
    }

    fn two_speaker_transcript() -> (Transcript, Audio) {
        // 1 kHz audio: speaker 0 has value 0.9 in 0-1 s, speaker 1 -0.9 in
        // 1-2 s, speaker 2 is listed but never speaks.
        let audio = Audio {
            sample_rate: 1000,
            samples: [vec![0.9; 1000], vec![-0.9; 1000]].concat(),
        };
        let seg = |start_ms, speaker| Segment {
            start_ms,
            end_ms: start_ms + 1000,
            speaker: Some(SpeakerId(speaker)),
            text: String::new(),
        };
        let t = Transcript {
            speakers: (0..3)
                .map(|id| Speaker {
                    id: SpeakerId(id),
                    name: None,
                })
                .collect(),
            segments: vec![seg(0, 0), seg(1000, 1)],
            ..Transcript::default()
        };
        (t, audio)
    }

    #[test]
    fn speaker_audio_joins_and_caps() {
        let (mut t, audio) = two_speaker_transcript();
        assert_eq!(
            speaker_audio(&audio, &t, SpeakerId(1)).samples,
            vec![-0.9; 1000]
        );
        assert!(speaker_audio(&audio, &t, SpeakerId(2)).samples.is_empty());
        // Segments past the end of the audio are clipped, the total is capped.
        t.segments[0].end_ms = 60_000;
        assert_eq!(speaker_audio(&audio, &t, SpeakerId(0)).samples.len(), 2000);
        let long = Audio {
            sample_rate: 1000,
            samples: vec![0.1; 40_000],
        };
        t.segments[0].end_ms = 40_000;
        assert_eq!(
            speaker_audio(&long, &t, SpeakerId(0)).samples.len(),
            MAX_SPEECH_MS as usize
        );
    }

    #[test]
    fn recognize_names_each_profile_once() {
        let (mut t, audio) = two_speaker_transcript();
        let mut p = VoiceProfiles::default();
        // Anna sounds like speaker 0; Bas is between both but closer to 1.
        p.remember("Anna", MODEL, &[0.9, 0.1]);
        p.remember("Bas", MODEL, &[-0.9, 0.1]);
        let named = recognize(
            &mut t,
            &audio,
            &mut MeanEmbedder,
            &p,
            MODEL,
            MATCH_THRESHOLD,
        )
        .unwrap();
        let names: Vec<_> = named.iter().map(|(id, n, _)| (id.0, n.as_str())).collect();
        assert_eq!(names, vec![(0, "Anna"), (1, "Bas")]);
        assert_eq!(
            t.speaker(SpeakerId(0)).unwrap().name.as_deref(),
            Some("Anna")
        );

        // With any similarity accepted, both speakers match Anna: only the
        // closer one (0) gets the name.
        let (mut t, audio) = two_speaker_transcript();
        let mut only_anna = VoiceProfiles::default();
        only_anna.remember("Anna", MODEL, &[0.9, 0.1]);
        let named = recognize(&mut t, &audio, &mut MeanEmbedder, &only_anna, MODEL, -1.0).unwrap();
        assert_eq!(named.len(), 1);
        assert_eq!(named[0].0, SpeakerId(0));
        assert_eq!(t.speaker(SpeakerId(1)).unwrap().name, None);
    }

    #[test]
    fn recognize_skips_named_speakers() {
        let (mut t, audio) = two_speaker_transcript();
        t.rename_speaker(SpeakerId(0), "Kees").unwrap();
        let mut p = VoiceProfiles::default();
        p.remember("Anna", MODEL, &[0.9, 0.1]);
        let named = recognize(
            &mut t,
            &audio,
            &mut MeanEmbedder,
            &p,
            MODEL,
            MATCH_THRESHOLD,
        )
        .unwrap();
        assert!(named.is_empty());
        assert_eq!(
            t.speaker(SpeakerId(0)).unwrap().name.as_deref(),
            Some("Kees")
        );
    }

    #[test]
    fn recognize_passes_embedder_errors_on() {
        let (mut t, audio) = two_speaker_transcript();
        let err = recognize(
            &mut t,
            &audio,
            &mut FailingEmbedder,
            &VoiceProfiles::default(),
            MODEL,
            MATCH_THRESHOLD,
        )
        .unwrap_err();
        assert_eq!(err.0, "model broke");
    }
}
