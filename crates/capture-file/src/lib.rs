//! Audio files as input. WAV only for now; compressed formats follow.
//!
//! The sample rate is kept as it is in the file. Resampling to what an engine
//! needs is the engine adapter's job, since engines differ.

use std::{io::Read, path::Path};

use hound::{SampleFormat, WavReader};
use saywut_core::asr::Audio;

pub use hound::Error as WavError;

/// Reads a WAV file and mixes all channels down to mono.
pub fn read_wav(path: &Path) -> Result<Audio, WavError> {
    read_wav_from(WavReader::open(path)?)
}

pub fn read_wav_from<R: Read>(reader: WavReader<R>) -> Result<Audio, WavError> {
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        SampleFormat::Float => reader.into_samples::<f32>().collect::<Result<_, _>>()?,
        SampleFormat::Int => {
            // hound accepts any multiple of 8 that fits the block size, but
            // only reads up to 32 bits; wider would also overflow the shift.
            if spec.bits_per_sample > 32 {
                return Err(WavError::Unsupported);
            }
            // Full scale of a signed integer with this many bits.
            let scale = (1_i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .into_samples::<i32>()
                .map(|s| s.map(|s| s as f32 / scale))
                .collect::<Result<_, _>>()?
        }
    };
    Ok(Audio {
        sample_rate: spec.sample_rate,
        samples: downmix(&samples, spec.channels),
    })
}

/// Averages interleaved frames into one channel. A trailing partial frame is
/// dropped.
fn downmix(interleaved: &[f32], channels: u16) -> Vec<f32> {
    if channels <= 1 {
        return interleaved.to_vec();
    }
    interleaved
        .chunks_exact(usize::from(channels))
        .map(|frame| frame.iter().sum::<f32>() / f32::from(channels))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use hound::{WavSpec, WavWriter};
    use std::io::Cursor;

    fn wav<S: hound::Sample + Copy>(spec: WavSpec, samples: &[S]) -> WavReader<Cursor<Vec<u8>>> {
        let mut buf = Cursor::new(Vec::new());
        let mut w = WavWriter::new(&mut buf, spec).unwrap();
        for s in samples {
            w.write_sample(*s).unwrap();
        }
        w.finalize().unwrap();
        buf.set_position(0);
        WavReader::new(buf).unwrap()
    }

    fn spec(channels: u16, bits: u16, format: SampleFormat) -> WavSpec {
        WavSpec {
            channels,
            sample_rate: 16_000,
            bits_per_sample: bits,
            sample_format: format,
        }
    }

    #[test]
    fn int16_mono_scales_to_unit_range() {
        let audio =
            read_wav_from(wav(spec(1, 16, SampleFormat::Int), &[0_i16, 16384, -32768])).unwrap();
        assert_eq!(audio.sample_rate, 16_000);
        assert_eq!(audio.samples, vec![0.0, 0.5, -1.0]);
    }

    #[test]
    fn int24_scales_by_its_own_width() {
        let audio = read_wav_from(wav(spec(1, 24, SampleFormat::Int), &[4_194_304_i32])).unwrap();
        assert_eq!(audio.samples, vec![0.5]);
    }

    #[test]
    fn float_stereo_is_averaged() {
        let audio = read_wav_from(wav(
            spec(2, 32, SampleFormat::Float),
            &[1.0_f32, 0.0, -0.5, -0.5],
        ))
        .unwrap();
        assert_eq!(audio.samples, vec![0.5, -0.5]);
    }

    #[test]
    fn downmix_drops_partial_frame() {
        assert_eq!(downmix(&[0.25, 0.75, 0.5], 2), vec![0.5]);
    }

    #[test]
    fn missing_file_is_an_error() {
        assert!(read_wav(Path::new("does/not/exist.wav")).is_err());
    }

    #[test]
    fn reads_a_file_from_disk() {
        let path =
            std::env::temp_dir().join(format!("saywut-capture-file-{}.wav", std::process::id()));
        let mut w = WavWriter::create(&path, spec(1, 16, SampleFormat::Int)).unwrap();
        w.write_sample(8192_i16).unwrap();
        w.finalize().unwrap();
        let audio = read_wav(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(audio.samples, vec![0.25]);
    }

    #[test]
    fn int_wider_than_32_bits_is_rejected() {
        // Hand-built header: PCM, mono, 9-byte blocks of 72-bit samples.
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36_u32 + 9).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16_u32.to_le_bytes());
        b.extend_from_slice(&1_u16.to_le_bytes()); // PCM
        b.extend_from_slice(&1_u16.to_le_bytes()); // mono
        b.extend_from_slice(&16_000_u32.to_le_bytes());
        b.extend_from_slice(&(16_000_u32 * 9).to_le_bytes());
        b.extend_from_slice(&9_u16.to_le_bytes()); // block align
        b.extend_from_slice(&72_u16.to_le_bytes()); // bits per sample
        b.extend_from_slice(b"data");
        b.extend_from_slice(&9_u32.to_le_bytes());
        b.extend_from_slice(&[0; 9]);
        let reader = WavReader::new(Cursor::new(b)).unwrap();
        assert!(matches!(read_wav_from(reader), Err(WavError::Unsupported)));
    }

    #[test]
    fn truncated_data_is_an_error() {
        let mut buf = Cursor::new(Vec::new());
        let mut w = WavWriter::new(&mut buf, spec(1, 16, SampleFormat::Int)).unwrap();
        w.write_sample(1_i16).unwrap();
        w.write_sample(2_i16).unwrap();
        w.finalize().unwrap();
        let mut bytes = buf.into_inner();
        bytes.pop(); // half a sample left at the end
        let reader = WavReader::new(Cursor::new(bytes)).unwrap();
        assert!(read_wav_from(reader).is_err());
    }
}
