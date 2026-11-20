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

Light and dark theme, following the OS setting with a manual override (lower
priority than the rest of phase 5).

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
- Languages are data, not code: BCP-47 tags (`nl`, `nl-BE`, `de-CH`, …), the
  speech languages offered come from what the active engine reports, and the
  speaker-naming phrases live in a per-language data file. Adding a language
  means adding files (UI locale, naming phrases, regression clips), not changing
  the core.
- UI: Tauri 2.

## Testing

Spec first: a failing test, then the code (red, green).

- Unit: pure core logic, `proptest` for edge cases.
- Component: pipeline with fake engine; real small model on short clips with
  `insta` snapshots.
- E2E: `tauri-driver` + WebdriverIO, fed by `capture-file`.
- Smoke: CI builds on all three OSes and transcribes a 5 s clip with the CLI.
- Regression: fixed clips per language with WER and DER budgets (slow, opt-in).
- Coverage: 100% lines, enforced in CI (Rust via cargo-llvm-cov, the frontend
  via Vitest coverage thresholds at 100). Only the call that opens an OS device
  may be excluded; those paths are on the manual checklist below.
- i18n: every key exists in all three locales.

## Dependency updates

- Dependabot, weekly on Monday: one grouped PR for minor and patch updates per
  ecosystem (Cargo, GitHub Actions; npm once the Tauri frontend exists), a
  separate PR per major update.
- A PR is merged only with green CI; no auto-merge.
- Model files are pinned by hash and updated by hand, since an update can change
  accuracy: rerun the regression budgets first. See Models below.

## Models

Models are data, like languages: one manifest, `models.toml` in the repository,
lists every model the app knows. Adding or updating a model is an edit there,
not a code change.

- **One entry per model file**: a stable `id` (`whisper-base`), the engine, the
  source URL pinned to an immutable revision (a Hugging Face commit, never
  `main`), the sha256, the size, the license and the languages it covers.
- **Content-addressed storage**: a file is stored as
  `<engine>/<id>-<sha256 first 12>.<ext>` under the model directory (the user's
  data directory in the app, `models/` in the repository for development). Old
  and new versions can sit side by side, so a rollback needs no download.
- **Verified download**: to a `.part` file, sha256 and size checked, then
  renamed. A mismatch deletes the file and is an error. A file is also re-hashed
  when it is first loaded after a start, so a corrupted file is noticed.
- **Never automatic**: the app downloads only when the user picks a model, and
  shows the size first. No background updates, no network access otherwise.
  The one bundled model (see Distribution) is verified against the manifest
  like any other.
- **Provenance in the transcript**: the transcript JSON records the engine,
  model id and sha256 that produced it, so a result can be reproduced and an
  accuracy change traced to a model update. (An optional field, so format
  version 1 stays readable.)
- **Updating a model**: change revision, sha256 and size in the manifest in one
  commit (`chore(models): …`), with the WER/DER before and after in the commit
  message. Dependabot does not cover this.
- **Tests gate a release**: a CI job runs the regression clips through every
  model in the manifest and fails when one exceeds its WER/DER budget. It runs
  on every change to `models.toml` (blocking the merge) and on every release
  tag (blocking the release), so no release ships a model that has not passed
  on exactly that file. Model files are cached in CI by sha256, so only a new
  or changed model is downloaded. Budgets live next to the model entry in the
  manifest.
  - CI runners have no GPU: models run on CPU there, so clips stay short.
    A model too slow for that runs the gate on a self-hosted runner instead.
  - The clips must be committed and freely licensed (own recordings or CC0,
    e.g. FLEURS or Common Voice; `testdata/speech/` holds the first ones);
    the Windows TTS clips cannot be made on Linux runners.
- **Pruning**: files not referenced by the manifest are only removed by an
  explicit command (`saywut models prune`), never silently.

## Slow machines

Transcription and diarization may not keep up with real time on a machine
without a usable GPU. Recording must never depend on that.

- **Record first, always**: audio goes to disk in short chunks (e.g. 30 s
  Opus/FLAC segments per track) as it comes in, independent of the engines. A
  crash or a slow engine loses no audio; the chunks are joined on Stop.
- **Measure, then choose a mode**: at start, run the engine on a few seconds
  and take the real-time factor (processing time / audio time). Below ~0.5:
  live. Between: live with a queue and a visible "n s behind" indicator. Above
  1: record now, transcribe afterwards. The user can override.
- **Live transcription is optional**: a toggle turns it off, leaving only the
  dB meters while recording. On Stop, transcription and diarization run with
  a progress bar (per phase: transcribing, diarizing) and a time estimate.
- **Afterwards with progress**: queued chunks are processed in the background
  with a progress bar and an estimate of the time left; the transcript fills
  in as chunks finish. The queue survives closing the app and resumes.
- **Less work per second**: voice activity detection skips silence; live can
  use a small model (e.g. whisper-base) and the after-pass a larger one.
- **Diarization in two steps**, as already planned: cheap provisional labels
  live, the full pass after Stop as a background job with its own progress.

## Reliability and crash logging

The one thing that must survive any failure is the recording.

- **Engines in a worker process**: ASR and diarization (C++ via whisper.cpp,
  ONNX) run in a `saywut-worker` child process. A native crash there kills
  only the worker; the app keeps recording, restarts it, and resumes from the
  chunk queue (see Slow machines). Capture and the chunk writer stay in the
  main process and are plain Rust.
- **Recovery on start**: an unfinished session (chunks without a closed
  transcript) is offered for recovery: join the chunks, transcribe what is
  missing.
- **Logs**: `tracing` to a rotating file in the user's log directory, a few
  days kept. No transcript text and no audio in logs; they hold events,
  timings, device names and errors.
- **Panics**: a panic hook writes the message, backtrace, version and OS to a
  crash file next to the logs, and the UI offers to open that folder on the
  next start.
- **Native crashes**: an out-of-process minidump handler (e.g. Embark's
  `crash-handler` + `minidumper`) writes a minidump for the worker or the app.
  Release builds keep their debug symbols outside the installer (`.pdb` on
  Windows, `.dSYM` on macOS, split DWARF on Linux), attached to the GitHub
  release, so a dump can be symbolicated afterwards.
- **No automatic upload**: logs and dumps stay local. Sending one is the
  user's choice, e.g. attaching it to an issue; a crash service would be
  opt-in only.
- **Failsafes in the data path**:
  - Malformed input (audio, JSON, manifest) returns an error, never panics;
    parsers get `cargo fuzz` targets, run in CI for a short time.
  - Size limits on everything read whole (manifest, transcript JSON); audio is
    streamed, not loaded at once.
  - A capture device that disappears marks that source as lost; the other
    tracks keep recording and the source reconnects when it returns.
  - Free disk space is checked at start and while recording; when it runs
    low, recording stops cleanly with every chunk finalized and the user is
    told why.

## Distribution

Tauri 2's bundler builds every format below from one config. Releases are built
only by CI, never on a developer machine:

- A `v*` tag starts a release workflow: the model gate first, then one job per
  OS (`tauri-apps/tauri-action`) that builds, signs and uploads the installers
  to a **draft** GitHub Release. Publishing the draft stays a manual step.
- Each binary gets a GitHub artifact attestation (build provenance), so anyone
  can verify it was built from that tag by that workflow.
- Signing keys live only in GitHub secrets (Apple Developer ID and notarization
  credentials, the Windows signing account, the updater key). Until they
  exist, releases are unsigned and marked as such.
- The build jobs install the Vulkan SDK; a CUDA build, if offered, is a
  separate job (Windows and Linux only).

- **macOS**: `.app` in a `.dmg` (drag to Applications). Must be signed with a
  Developer ID and notarized by Apple, or Gatekeeper blocks it; this needs a
  paid Apple Developer account. The Core Audio tap needs its usage description
  and entitlement in the bundle. Universal binary (arm64 + x86_64).
- **Windows**: NSIS `.exe` installer (per-user, no admin needed) as default;
  MSI only if someone needs it for managed deployment. Tauri's installer
  bootstraps WebView2 when missing. Unsigned builds trigger SmartScreen
  warnings: sign with a code-signing certificate (Azure Trusted Signing is the
  cheapest). Later also a winget manifest.
- **Linux**: AppImage (one file, runs on most distros) plus `.deb`; `.rpm` and
  Flatpak later. Per-app capture talks to PipeWire, which works outside a
  sandbox; Flatpak would need the PipeWire portal permissions.
- **Models**: the installer carries only the smallest usable model (a quantized
  whisper-base, about 60 MB), so the app works offline right after install.
  Every other model is downloaded on demand when the user picks it (see
  Models), with its size shown first.
- **Model choice**: per model the UI shows size, languages and a speed
  measured on this machine (a short trial run), and recommends one based on
  the hardware found (e.g. a GPU: large-v3-turbo). Live and the after-pass can
  use different models: a small fast one live, a larger one after Stop.
- **GPU**: the default build carries the Vulkan backend (only the driver
  needed). CUDA, if offered, is a separate download of the extra runtime
  libraries, not part of every installer.
- **Updates**: Tauri's updater with signed update manifests, opt-in, and never
  touching models.

## Phases

1. Spike: compare whisper, parakeet and voxtral on own nl/en/de recordings (WER,
   speed on an RTX 3070 Laptop); check per-app capture of Opera GX.
   - Candidates: whisper base, medium and large-v3-turbo (and a Dutch
     fine-tune), Parakeet TDT 0.6B v3, Voxtral Mini Transcribe and Voxtral
     Realtime (low delay, interesting for live), and Vosk (Kaldi, light and
     truly streaming: a candidate for live on machines without a GPU, with a
     larger model doing the after-pass). Cloud engines such as ElevenLabs
     Scribe only if the open question on cloud APIs is answered yes.
   - Data: Common Voice nl/en/de (CC0, read speech) and own recordings
     (spontaneous speech), including video calls: compressed audio, uneven
     microphone quality between speakers, people talking over each other. Published Dutch results show whisper doing clearly
     worse on spontaneous broadcast speech than on Common Voice, so read speech
     alone flatters every model.
   - No independent Dutch comparison of these engines was found; Mistral's
     Voxtral figures are its own.
   - Backends per engine: CPU, Vulkan, CUDA (`tools/bench-whisper.sh`).
2. Core and CLI: engine traits, file in, JSON/SRT out. i18n from the start.
3. Diarization over a whole file, speaker renaming. 3b: names from speech and
   voice profiles.
4. Live from the microphone: VAD, chunking, partial/final lines, dB meter, writing
   audio and JSON while recording.
5. Tauri desktop UI.
6. More sources: device loopback and per-app capture, multitrack.

## Reference projects

Looked at for ideas, no code taken:

- thewh1teagle/vibe: closest to Saywut (Tauri + whisper, offline).
- Zackriya-Solutions/meetily: capturing and mixing several audio sources.
- 0xkaz/rust-whisper-diarization: API shape only, a small demo.

## Open questions

- Local only, or is a cloud API (e.g. Voxtral's) allowed as an option?
- GPU backend in the installer: Vulkan (any vendor, only the driver needed,
  small installer) as default with CUDA optional (faster on NVIDIA, but the
  redistributable cuBLAS/cudart DLLs add hundreds of MB)? Measure both on the
  RTX 3070 Laptop in the spike. The CUDA toolkit or Vulkan SDK is only needed
  on the build machine, not by users.
  First measurement (2026-11-20, whisper-base, release CLI incl. model load,
  `tools/bench-whisper.sh`, real-time factor on the three TTS clips): CPU
  0.22–0.62, Vulkan 0.04–0.08, CUDA 0.04–0.10, same text on all three. For a
  small model the backends are equal; repeat with large-v3-turbo.

## Manual checks (need real hardware or accounts)

- Per-app capture of Opera GX on Windows (process loopback including child
  processes).
- Core Audio tap permission flow on macOS.
- Accuracy on own nl/en/de recordings for the spike.
- Install a German voice and a second Dutch voice (Settings > Time & language >
  Speech), so `tools/tts-clips.ps1` can also make `de` clips and a Dutch
  dialogue.
- Diarization with three or more real voices: the TTS clips cannot show it
  (two male Microsoft voices merge into one speaker).
- Voice profiles on real voices: does `transcribe --recognize` name the right
  people, and is `MATCH_THRESHOLD` (0.6) right for CAM++ on real speech?
- Desktop app against the fake session (`npm run tauri dev` in
  `crates/saywut-app`): the panel looks right next to a browser at its narrow
  width; "On top" keeps it above a focused window; the meter moves with the
  speech; partial lines are italic and become final; light and dark follow
  the OS setting, also when it changes while the app runs; nl and de via
  `SAYWUT_LANG`. On Windows, macOS and Linux.
- The app opening its window (`saywut-app/src/main.rs`, outside coverage).
