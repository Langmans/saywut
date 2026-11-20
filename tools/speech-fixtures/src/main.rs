//! Builds the committed speech clips in `testdata/speech/` from FLEURS
//! (google/fleurs, CC-BY 4.0) recordings.
//!
//! ```sh
//! cargo run --manifest-path tools/speech-fixtures/Cargo.toml -- <fleurs-dir> testdata/speech
//! ```
//!
//! `<fleurs-dir>` holds `<lang>/dev.tsv` and `<lang>/dev/<file>.wav` from the
//! dataset's `dev` split (revision 70bb2e84b976b7e960aa89f1c648e09c59f894dd),
//! with at least the files named below extracted. Output is 16 kHz mono
//! 16-bit WAV plus a transcript JSON per clip:
//!
//! - `nl.wav`, `en.wav`, `de.wav`: one sentence each, with its exact text,
//!   for recognition tests.
//! - `two-voices.wav`: two Dutch recordings by different people. Each is cut
//!   at its quietest point near the middle and the halves alternate A, B, A,
//!   B, so both voices speak twice. Only the times and speakers of its JSON
//!   are a reference; the text of a half is the whole sentence marked with
//!   the half.

use std::{fs, path::Path};

use saywut_core::{
    asr::Audio,
    level::rms_dbfs,
    transcript::{Segment, Speaker, SpeakerId, Transcript},
};

const RATE: u32 = 16_000;
/// Silence between lines, as in the TTS clips.
const GAP_MS: u64 = 600;

/// (language folder, output name, BCP-47 tag, file)
const SINGLES: [(&str, &str, &str, &str); 3] = [
    ("nl_nl", "nl", "nl", "15285589950436800967.wav"),
    ("en_us", "en", "en", "9742912202579443140.wav"),
    ("de_de", "de", "de", "7146016300570749740.wav"),
];

/// Two Dutch recordings by different people (sentences 1550 and 1594),
/// checked with `--probe`: CAM++ similarity about 0.2 between them against
/// about 0.8 within each. Two readers of one FLEURS sentence can be the same
/// person, so the probe is needed.
const TWO_VOICES: [&str; 2] = ["9569918354716171450.wav", "8132718264905775195.wav"];

/// Peak level every recording is scaled to; many FLEURS recordings peak
/// around 0.01-0.04 and are hard to hear.
const PEAK: f32 = 0.9;

/// Reads a recording and scales it to [`PEAK`].
fn read(dir: &Path, lang: &str, file: &str) -> Audio {
    let mut audio = saywut_capture_file::read_wav(&dir.join(lang).join("dev").join(file))
        .unwrap_or_else(|e| panic!("{lang}/dev/{file}: {e}"));
    assert_eq!(audio.sample_rate, RATE, "{file}");
    let peak = audio.samples.iter().fold(0.0_f32, |m, s| m.max(s.abs()));
    for s in &mut audio.samples {
        *s *= PEAK / peak;
    }
    audio
}

/// The raw (cased, punctuated) transcription of `file` from `dev.tsv`.
fn text_of(dir: &Path, lang: &str, file: &str) -> String {
    let tsv = fs::read_to_string(dir.join(lang).join("dev.tsv")).unwrap();
    tsv.lines()
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .find(|cols| cols[1] == file)
        .unwrap_or_else(|| panic!("{file} not in {lang}/dev.tsv"))[2]
        .to_owned()
}

fn write(out: &Path, name: &str, samples: &[f32], transcript: &Transcript) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(out.join(format!("{name}.wav")), spec).unwrap();
    for s in samples {
        w.write_sample((s.clamp(-1.0, 1.0) * 32767.0) as i16).unwrap();
    }
    w.finalize().unwrap();
    fs::write(out.join(format!("{name}.json")), transcript.to_json() + "\n").unwrap();
    println!("{name}.wav  {:.1} s", samples.len() as f64 / f64::from(RATE));
}

fn ms(samples: usize) -> u64 {
    samples as u64 * 1000 / u64::from(RATE)
}

/// Sample index of the quietest 50 ms window between 35% and 65% of `audio`.
fn quietest_middle(audio: &Audio) -> usize {
    let win = (RATE / 20) as usize;
    let n = audio.samples.len();
    (n * 35 / 100..n * 65 / 100)
        .step_by(win / 2)
        .min_by(|&a, &b| {
            rms_dbfs(&audio.samples[a..a + win]).total_cmp(&rms_dbfs(&audio.samples[b..b + win]))
        })
        .map(|start| start + win / 2)
        .unwrap()
}

/// `--probe <fleurs-dir> <file>...`: voice similarity between Dutch
/// recordings, to pick two that are really different people. FLEURS's
/// "several readers per sentence" is no guarantee: one volunteer can read a
/// sentence twice. Prints, per pair, the cosine similarity of the CAM++
/// embeddings (first half of one against second half of the other, so a
/// recording against itself shows what "same voice" looks like).
fn probe(dir: &Path, files: &[String]) {
    use saywut_core::profiles::{SpeakerEmbedder, cosine};
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest = saywut_models::Manifest::builtin();
    let model = manifest.get("campplus-voxceleb").unwrap().path(&repo.join("models"));
    let mut embedder = saywut_diarize_sherpa::SherpaEmbedder::load(&model).unwrap();
    let halves: Vec<[Vec<f32>; 2]> = files
        .iter()
        .map(|f| {
            let a = read(dir, "nl_nl", f);
            let cut = quietest_middle(&a);
            [&a.samples[..cut], &a.samples[cut..]].map(|part| {
                let half = Audio { sample_rate: RATE, samples: part.to_vec() };
                embedder.embed(&half).unwrap()
            })
        })
        .collect();
    for (i, a) in halves.iter().enumerate() {
        let row: Vec<String> = halves
            .iter()
            .map(|b| format!("{:5.2}", cosine(&a[0], &b[1])))
            .collect();
        println!("{:>22} {}", files[i], row.join(" "));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args[1] == "--probe" {
        return probe(Path::new(&args[2]), &args[3..]);
    }
    let (dir, out) = (Path::new(&args[1]), Path::new(&args[2]));
    fs::create_dir_all(out).unwrap();

    for (lang, name, tag, file) in SINGLES {
        let audio = read(dir, lang, file);
        let t = Transcript {
            language: Some(tag.to_owned()),
            speakers: vec![Speaker { id: SpeakerId(0), name: None }],
            segments: vec![Segment {
                start_ms: 0,
                end_ms: ms(audio.samples.len()),
                speaker: Some(SpeakerId(0)),
                text: text_of(dir, lang, file),
            }],
            ..Transcript::default()
        };
        write(out, name, &audio.samples, &t);
    }

    let voices: Vec<Audio> = TWO_VOICES.iter().map(|f| read(dir, "nl_nl", f)).collect();
    let texts: Vec<String> = TWO_VOICES.iter().map(|f| text_of(dir, "nl_nl", f)).collect();
    let halves: Vec<[&[f32]; 2]> = voices
        .iter()
        .map(|a| {
            let cut = quietest_middle(a);
            [&a.samples[..cut], &a.samples[cut..]]
        })
        .collect();
    let gap = vec![0.0; (GAP_MS * u64::from(RATE) / 1000) as usize];
    let (mut samples, mut segments) = (Vec::new(), Vec::new());
    for part in 0..2 {
        for (speaker, voice) in halves.iter().enumerate() {
            let start = ms(samples.len());
            samples.extend_from_slice(voice[part]);
            segments.push(Segment {
                start_ms: start,
                end_ms: ms(samples.len()),
                speaker: Some(SpeakerId(speaker as u32)),
                text: format!("({}/2) {}", part + 1, texts[speaker]),
            });
            samples.extend_from_slice(&gap);
        }
    }
    let t = Transcript {
        language: Some("nl".into()),
        speakers: (0..2).map(|id| Speaker { id: SpeakerId(id), name: None }).collect(),
        segments,
        ..Transcript::default()
    };
    write(out, "two-voices", &samples, &t);
}
