//! Diarization ([`SherpaDiarizer`]: pyannote segmentation, CAM++ embeddings,
//! clustering) and voice embeddings for profiles ([`SherpaEmbedder`]) via
//! sherpa-onnx. Model paths come from the manifest; see the README.

use std::path::Path;

use saywut_core::{
    asr::Audio,
    diarize::{DiarizeError, Diarizer, Turn},
    profiles::SpeakerEmbedder,
    transcript::SpeakerId,
};
use sherpa_rs::{
    diarize::{Diarize, DiarizeConfig},
    speaker_id::{EmbeddingExtractor, ExtractorConfig},
};

/// The models are trained on 16 kHz mono.
pub const SAMPLE_RATE: u32 = 16_000;

/// Cosine-distance threshold for clustering when the number of speakers is
/// not given: lower splits voices into more speakers. sherpa-onnx's default.
pub const CLUSTER_THRESHOLD: f32 = 0.5;

/// Loaded segmentation and embedding models.
pub struct SherpaDiarizer {
    inner: Diarize,
}

fn error(e: impl std::fmt::Display) -> DiarizeError {
    DiarizeError(e.to_string())
}

/// sherpa-rs unwraps non-UTF-8 paths and loads missing files lazily; check up
/// front so the error says which file is the problem.
fn check_model(path: &Path) -> Result<&str, DiarizeError> {
    path.to_str()
        .filter(|_| path.is_file())
        .ok_or_else(|| error(format!("model file not usable: {}", path.display())))
}

fn check_rate(audio: &Audio) -> Result<(), DiarizeError> {
    if audio.sample_rate != SAMPLE_RATE {
        return Err(error(format!(
            "needs {SAMPLE_RATE} Hz audio, this is {} Hz",
            audio.sample_rate
        )));
    }
    Ok(())
}

impl SherpaDiarizer {
    /// `speakers`: the number of speakers if known, else they are found by
    /// clustering with [`CLUSTER_THRESHOLD`].
    pub fn load(
        segmentation: &Path,
        embedding: &Path,
        speakers: Option<u32>,
    ) -> Result<Self, DiarizeError> {
        check_model(segmentation)?;
        check_model(embedding)?;
        let config = DiarizeConfig {
            // sherpa-onnx clusters by threshold when the count is not positive.
            num_clusters: Some(speakers.map_or(-1, |n| n as i32)),
            threshold: Some(CLUSTER_THRESHOLD),
            ..DiarizeConfig::default()
        };
        let inner = Diarize::new(segmentation, embedding, config).map_err(error)?;
        Ok(Self { inner })
    }
}

impl Diarizer for SherpaDiarizer {
    fn diarize(&mut self, audio: &Audio) -> Result<Vec<Turn>, DiarizeError> {
        check_rate(audio)?;
        let segments = self
            .inner
            .compute(audio.samples.clone(), None)
            .map_err(error)?;
        Ok(segments
            .into_iter()
            .map(|s| Turn {
                start_ms: (s.start.max(0.0) * 1000.0) as u64,
                end_ms: (s.end.max(0.0) * 1000.0) as u64,
                speaker: SpeakerId(s.speaker.max(0) as u32),
            })
            .collect())
    }
}

/// Voice embeddings for voice profiles, from the same model diarization
/// clusters with.
pub struct SherpaEmbedder {
    inner: EmbeddingExtractor,
}

impl SherpaEmbedder {
    /// Loads a speaker-embedding model file.
    pub fn load(model: &Path) -> Result<Self, DiarizeError> {
        let config = ExtractorConfig {
            model: check_model(model)?.to_owned(),
            ..ExtractorConfig::default()
        };
        let inner = EmbeddingExtractor::new(config).map_err(error)?;
        Ok(Self { inner })
    }
}

impl SpeakerEmbedder for SherpaEmbedder {
    fn embed(&mut self, audio: &Audio) -> Result<Vec<f32>, DiarizeError> {
        check_rate(audio)?;
        self.inner
            .compute_speaker_embedding(audio.samples.clone(), SAMPLE_RATE)
            .map_err(error)
    }
}
