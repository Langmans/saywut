# speech-fixtures

Builds the committed speech clips in `testdata/speech/` from FLEURS
(google/fleurs, CC BY 4.0). A tool of its own, outside the workspace; run by
hand when the clips change.

```sh
cargo run --manifest-path tools/speech-fixtures/Cargo.toml -- <fleurs-dir> testdata/speech
cargo run --manifest-path tools/speech-fixtures/Cargo.toml -- --probe <fleurs-dir> <file>...
```

`<fleurs-dir>` holds `<lang>/dev.tsv` and `<lang>/dev/<file>.wav` from the
dataset's `dev` split (revision `70bb2e84b976b7e960aa89f1c648e09c59f894dd`),
with at least the files named in `src/main.rs` extracted. Output is 16 kHz
mono 16-bit WAV plus a transcript JSON per clip, each recording scaled to a
0.9 peak:

- `nl.wav`, `en.wav`, `de.wav`: one sentence each, with its exact text, for
  recognition tests.
- `two-voices.wav`: two Dutch recordings by different people. Each is cut at
  its quietest point near the middle and the halves alternate A, B, A, B, so
  both voices speak twice. Only the times and speakers of its JSON are a
  reference; the text of a half is the whole sentence marked with the half.

`--probe` prints the CAM++ voice similarity between recordings (first half
of one against second half of the other), to check that two recordings are
really different people: two readers of one FLEURS sentence can be the same
person. Building the probe needs `target/debug/deps` in this folder, as for
the workspace.
