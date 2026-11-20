# Saywut plan

## Goal

- Live transcription with speaker diarization while people speak.
- Sources: microphone, output-device loopback, single application (e.g. a browser
  playing a stream). Several at once, one track per source, a dB meter per source.
- Output: audio (`.opus`/`.flac`, one track per source) and transcript JSON as the
  source of truth, exported to SRT, VTT, TXT and Markdown.
- From an existing audio file: transcript only.
- Speech languages nl, en, de (auto or fixed per recording); UI i18n en, nl, de.
- Desktop only: Windows, macOS, Linux.

## UI

A narrow panel that sits next to a separate browser, with an always-on-top toggle:
source chips, a horizontal meter per source, the scrolling transcript with speaker
labels. Partial lines are shown in italics until final. No embedded browser.

## Architecture

Ports and adapters. `saywut-core` holds the traits and all pure logic; adapters are
thin crates.

- `AudioSource`: `capture-cpal` (mic), `capture-wasapi` (Windows loopback and
  process loopback), `capture-coreaudio` (macOS Core Audio taps),
  `capture-pipewire` (Linux per-app nodes), `capture-file` (a file played as a
  live source, also used for file transcription and e2e tests), `capture-fake`.
- `AsrEngine`: whisper (`whisper-rs`), parakeet (`parakeet-rs`/`transcribe-rs`),
  voxtral realtime, fake.
- `Diarizer`: sherpa-onnx, sortformer, fake. Live labels are provisional; after
  Stop the whole recording is diarized again and labels are corrected.
- Speaker naming: phrases like "ik ben X", "I'm X", "ich bin X" become a suggestion
  the user confirms. A confirmed name stores a local voice profile (embedding),
  matched in later sessions. Voice profiles are biometric data: local only, opt-in,
  deletable.
- i18n: Fluent (`.ftl`) shared by Rust and the frontend.
- UI: Tauri 2.

## Testing

Spec first: a failing test, then the code (red, green).

- Unit: pure core logic, `proptest` for edge cases.
- Component: pipeline with fake engine; real small model on short clips with
  `insta` snapshots.
- E2E: `tauri-driver` + WebdriverIO, fed by `capture-file`.
- Smoke: CI builds on all three OSes and transcribes a 5 s clip with the CLI.
- Regression: fixed clips per language with WER and DER budgets (slow, opt-in).
- Coverage: 100% lines, enforced in CI. Only the call that opens an OS device may
  be excluded; those paths are on the manual checklist below.
- i18n: every key exists in all three locales.

## Phases

1. Spike: compare whisper, parakeet and voxtral on own nl/en/de recordings (WER,
   speed on an RTX 3070 Laptop); check per-app capture of Opera GX.
2. Core and CLI: engine traits, file in, JSON/SRT out. i18n from the start.
3. Diarization over a whole file, speaker renaming. 3b: names from speech and
   voice profiles.
4. Live from the microphone: VAD, chunking, partial/final lines, dB meter, writing
   audio and JSON while recording.
5. Tauri desktop UI.
6. More sources: device loopback and per-app capture, multitrack.

## Open questions

- Local only, or is a cloud API (e.g. Voxtral's) allowed as an option?
- License.

## Manual checks (need real hardware or accounts)

- Per-app capture of Opera GX on Windows (process loopback including child
  processes).
- Core Audio tap permission flow on macOS.
- Accuracy on own nl/en/de recordings for the spike.
