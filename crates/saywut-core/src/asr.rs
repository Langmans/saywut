//! Speech recognition port. Engines (whisper, parakeet, voxtral) implement
//! [`AsrEngine`] in their own adapter crates; [`FakeAsr`] lets the pipeline be
//! tested without a model or a recording.

use crate::transcript::Segment;

/// Mono PCM, samples in -1.0..=1.0.
#[derive(Debug, Clone, PartialEq)]
pub struct Audio {
    pub sample_rate: u32,
    pub samples: Vec<f32>,
}

impl Audio {
    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate == 0 {
            return 0;
        }
        self.samples.len() as u64 * 1000 / u64::from(self.sample_rate)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsrError {
    /// The engine does not offer this BCP-47 language.
    UnsupportedLanguage(String),
    /// Anything the engine itself reports, passed through as text.
    Engine(String),
}

impl std::fmt::Display for AsrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedLanguage(tag) => write!(f, "language not supported: {tag}"),
            Self::Engine(msg) => write!(f, "speech recognition failed: {msg}"),
        }
    }
}

impl std::error::Error for AsrError {}

pub trait AsrEngine {
    /// BCP-47 tags this engine can transcribe. The UI offers exactly these,
    /// so adding a language never touches the core.
    fn languages(&self) -> Vec<String>;

    /// Transcribe `audio`. `language` is `None` for auto-detection. Segment
    /// times are relative to the start of `audio`; speakers are left unset,
    /// as diarization is a separate step.
    fn transcribe(
        &mut self,
        audio: &Audio,
        language: Option<&str>,
    ) -> Result<Vec<Segment>, AsrError>;
}

/// Returns scripted segments, clipped to the length of the audio it is given.
#[derive(Debug, Clone, Default)]
pub struct FakeAsr {
    pub languages: Vec<String>,
    pub script: Vec<Segment>,
}

impl AsrEngine for FakeAsr {
    fn languages(&self) -> Vec<String> {
        self.languages.clone()
    }

    fn transcribe(
        &mut self,
        audio: &Audio,
        language: Option<&str>,
    ) -> Result<Vec<Segment>, AsrError> {
        if let Some(tag) = language
            && !self.languages.iter().any(|l| l == tag)
        {
            return Err(AsrError::UnsupportedLanguage(tag.to_owned()));
        }
        let end = audio.duration_ms();
        Ok(self
            .script
            .iter()
            .filter(|s| s.start_ms < end)
            .map(|s| Segment {
                end_ms: s.end_ms.min(end),
                ..s.clone()
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start_ms: u64, end_ms: u64, text: &str) -> Segment {
        Segment {
            start_ms,
            end_ms,
            speaker: None,
            text: text.into(),
        }
    }

    fn engine() -> FakeAsr {
        FakeAsr {
            languages: vec!["nl".into(), "en".into()],
            script: vec![
                seg(0, 800, "een"),
                seg(800, 1600, "twee"),
                seg(2000, 2500, "drie"),
            ],
        }
    }

    fn one_and_a_half_seconds() -> Audio {
        Audio {
            sample_rate: 16_000,
            samples: vec![0.0; 24_000],
        }
    }

    #[test]
    fn duration() {
        assert_eq!(one_and_a_half_seconds().duration_ms(), 1500);
        let broken = Audio {
            sample_rate: 0,
            samples: vec![0.0; 10],
        };
        assert_eq!(broken.duration_ms(), 0);
    }

    #[test]
    fn reports_languages() {
        assert_eq!(engine().languages(), vec!["nl", "en"]);
    }

    #[test]
    fn clips_script_to_audio_length() {
        let out = engine()
            .transcribe(&one_and_a_half_seconds(), Some("nl"))
            .unwrap();
        assert_eq!(out, vec![seg(0, 800, "een"), seg(800, 1500, "twee")]);
    }

    #[test]
    fn auto_language_is_accepted() {
        assert_eq!(
            engine()
                .transcribe(&one_and_a_half_seconds(), None)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn unsupported_language_is_rejected() {
        let err = engine()
            .transcribe(&one_and_a_half_seconds(), Some("de"))
            .unwrap_err();
        assert_eq!(err, AsrError::UnsupportedLanguage("de".into()));
        assert_eq!(err.to_string(), "language not supported: de");
    }

    #[test]
    fn engine_error_message() {
        assert_eq!(
            AsrError::Engine("out of memory".into()).to_string(),
            "speech recognition failed: out of memory"
        );
    }
}
