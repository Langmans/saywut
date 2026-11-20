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

## License

[MIT](LICENSE)
