# Saywut

Desktop app for live transcription with speaker diarization. It records from a
microphone, an output device or a single application, shows the transcript as
it is spoken next to a dB meter per source, and writes two files: the audio and
the transcript. An existing audio file can be transcribed as well.

Languages: speech in Dutch, English and German; UI in the same three.

Status: early. See [PLAN.md](PLAN.md) for the design and phases.

## System requirements

Targets, not yet measured; the spike (phase 1) will confirm or adjust them.

| | Windows | macOS | Linux |
|---|---|---|---|
| OS | Windows 10 (1809+) | macOS 10.15+ | glibc distro with WebKitGTK 4.1 |
| Per-app capture | Windows 11, or Windows 10 build 20348+ | macOS 14.2+ (Core Audio taps) | PipeWire |
| Runtime | WebView2 (installed by the installer if missing) | built in | WebKitGTK 4.1 |

- **CPU**: x86_64 or arm64 with AVX2 (x86_64) for usable speed on the CPU.
- **Memory**: about 1 GB free with whisper-base; larger models need more.
- **Disk**: the app plus the chosen models (whisper-base is 148 MB), plus the
  recordings themselves (Opus is about 1 MB per minute per track).
- **GPU (optional)**: any GPU with a Vulkan 1.2 driver; on NVIDIA optionally
  CUDA. Without a GPU, transcription runs on the CPU, slower than real time
  for larger models.
- **Network**: only to download a model the user picks. Recording and
  transcription work offline.

## Building

Requirements on Windows: Rust (stable, msvc), Visual Studio C++ build tools,
LLVM/Clang (`LIBCLANG_PATH` pointing at its `bin`) and CMake. For GPU builds
also the Vulkan SDK (`VULKAN_SDK`) or the CUDA Toolkit (`CUDA_PATH`); users
need neither.

```sh
cargo test --workspace
cargo llvm-cov --workspace --fail-under-lines 100
```

Coverage in the editor (Coverage Gutters reads `lcov.info`):

```sh
cargo llvm-cov --workspace --lcov --output-path lcov.info
```

## Test audio

Synthetic speech clips with an exact reference transcript, from the Windows
voices (Windows only):

```sh
pwsh -NoProfile -File tools/tts-clips.ps1
```

This writes `testdata/tts/<clip>.wav` (16 kHz mono) and `<clip>.json` in the
transcript format. A clip whose voice is not installed is skipped. The clips
are good for pipeline and diarization checks, not for measuring accuracy.

## License

[MIT](LICENSE)
