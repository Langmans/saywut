//! Whisper via whisper.cpp (`whisper-rs`), on the CPU for now.
//!
//! The model file comes from the manifest (`saywut-models`); this crate only
//! loads a path it is given. Whisper itself detects and supports many more
//! languages than Saywut offers: the list passed to [`WhisperAsr::load`] (the
//! manifest's `languages`) is what the UI shows and what is accepted.

use std::path::Path;

use saywut_core::{
    asr::{AsrEngine, AsrError, Audio},
    transcript::Segment,
};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Whisper models are trained on 16 kHz mono.
pub const SAMPLE_RATE: u32 = 16_000;

pub struct WhisperAsr {
    ctx: WhisperContext,
    languages: Vec<String>,
}

/// Whisper's language code for a BCP-47 tag: its primary subtag.
fn primary(tag: &str) -> &str {
    tag.split(['-', '_']).next().unwrap_or(tag)
}

fn engine_error(e: impl std::fmt::Display) -> AsrError {
    AsrError::Engine(e.to_string())
}

impl WhisperAsr {
    pub fn load(model: &Path, languages: Vec<String>) -> Result<Self, AsrError> {
        // Route whisper.cpp's own logging away from stderr; without a `log`
        // backend it is dropped.
        whisper_rs::install_logging_hooks();
        let ctx = WhisperContext::new_with_params(model, WhisperContextParameters::default())
            .map_err(engine_error)?;
        Ok(Self { ctx, languages })
    }
}

impl AsrEngine for WhisperAsr {
    fn languages(&self) -> Vec<String> {
        self.languages.clone()
    }

    fn transcribe(
        &mut self,
        audio: &Audio,
        language: Option<&str>,
    ) -> Result<Vec<Segment>, AsrError> {
        if audio.sample_rate != SAMPLE_RATE {
            return Err(AsrError::Engine(format!(
                "whisper needs {SAMPLE_RATE} Hz audio, this is {} Hz",
                audio.sample_rate
            )));
        }
        let code = match language {
            Some(tag) if self.languages.iter().any(|l| primary(l) == primary(tag)) => primary(tag),
            Some(tag) => return Err(AsrError::UnsupportedLanguage(tag.to_owned())),
            None => "auto",
        };

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some(code));
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        params.set_n_threads(i32::try_from(threads).unwrap_or(4));

        let mut state = self.ctx.create_state().map_err(engine_error)?;
        state.full(params, &audio.samples).map_err(engine_error)?;

        let mut segments = Vec::new();
        for i in 0..state.full_n_segments() {
            let seg = state.get_segment(i).expect("index below full_n_segments");
            let text = seg.to_str_lossy().map_err(engine_error)?.trim().to_owned();
            // whisper.cpp timestamps are in centiseconds.
            let ms = |t: i64| u64::try_from(t).unwrap_or(0) * 10;
            segments.push(Segment {
                start_ms: ms(seg.start_timestamp()),
                end_ms: ms(seg.end_timestamp()),
                speaker: None,
                text,
            });
        }
        Ok(segments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primary_subtag() {
        assert_eq!(primary("nl"), "nl");
        assert_eq!(primary("nl-BE"), "nl");
        assert_eq!(primary("de_CH"), "de");
    }

    #[test]
    fn missing_model_is_an_engine_error() {
        let err = WhisperAsr::load(Path::new("no/such/model.bin"), vec![])
            .err()
            .unwrap();
        assert!(matches!(err, AsrError::Engine(_)));
    }
}
