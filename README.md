# Saywut

Desktop app for live transcription with speaker diarization. It records from a
microphone, an output device or a single application, shows the transcript as
it is spoken next to a dB meter per source, and writes two files: the audio and
the transcript. An existing audio file can be transcribed as well.

Languages: speech in Dutch, English and German; UI in the same three.

Status: early. See [PLAN.md](PLAN.md) for the design and phases.

## Building

Requirements on Windows: Rust (stable, msvc), Visual Studio C++ build tools,
LLVM/Clang (`LIBCLANG_PATH` pointing at its `bin`) and CMake.

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
