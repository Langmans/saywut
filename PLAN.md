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
- Coverage: 100% lines, enforced in CI. Only the call that opens an OS device may
  be excluded; those paths are on the manual checklist below.
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
    e.g. Common Voice); the Windows TTS clips cannot be made on Linux runners.
- **Pruning**: files not referenced by the manifest are only removed by an
  explicit command (`saywut models prune`), never silently.

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
- **Not in the installer**: models. They are downloaded on first use after the
  user picks one (see Models), which keeps installers small and the user in
  control of what is downloaded.
- **GPU**: the default build carries the Vulkan backend (only the driver
  needed). CUDA, if offered, is a separate download of the extra runtime
  libraries, not part of every installer.
- **Updates**: Tauri's updater with signed update manifests, opt-in, and never
  touching models.

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
- GPU backend in the installer: Vulkan (any vendor, only the driver needed,
  small installer) as default with CUDA optional (faster on NVIDIA, but the
  redistributable cuBLAS/cudart DLLs add hundreds of MB)? Measure both on the
  RTX 3070 Laptop in the spike. The CUDA toolkit or Vulkan SDK is only needed
  on the build machine, not by users.

## Manual checks (need real hardware or accounts)

- Per-app capture of Opera GX on Windows (process loopback including child
  processes).
- Core Audio tap permission flow on macOS.
- Accuracy on own nl/en/de recordings for the spike.
- Install a German voice and a second Dutch voice (Settings > Time & language >
  Speech), so `tools/tts-clips.ps1` can also make `de` clips and a Dutch
  dialogue.
