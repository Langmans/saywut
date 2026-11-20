//! Platform-independent core of Saywut.
//!
//! Everything here is pure and fully unit-tested. OS capture, ASR engines and
//! diarizers live in adapter crates behind traits defined in this crate.

pub mod asr;
pub mod diarize;
pub mod export;
pub mod level;
pub mod pipeline;
pub mod transcript;
pub mod wer;
