# Speech test clips

Recordings from **FLEURS** (Google), licensed under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/):
<https://huggingface.co/datasets/google/fleurs>, revision
`70bb2e84b976b7e960aa89f1c648e09c59f894dd`, `dev` split.

Changes: each recording scaled to a peak of 0.9 (many FLEURS recordings
peak around 0.01–0.04), converted from 32-bit float to 16-bit samples;
`two-voices.wav` is cut and rearranged (see below). Built by
`tools/speech-fixtures`.

| Clip | Source files (`<lang>/dev/`) | Use |
|---|---|---|
| `nl.wav` | `nl_nl` 15285589950436800967 | Dutch recognition, exact text |
| `en.wav` | `en_us` 9742912202579443140 | English recognition, exact text |
| `de.wav` | `de_de` 7146016300570749740 | German recognition, exact text |
| `two-voices.wav` | `nl_nl` 9569918354716171450, 8132718264905775195 | diarization and voice profiles |

`two-voices.wav` holds two Dutch recordings by different people (FLEURS
sentences 1550 and 1594). That they are different people was checked with
voice embeddings (`tools/speech-fixtures --probe`: similarity about 0.2
between them, about 0.8 within each); two readers of one FLEURS sentence
can be the same person. Each recording is cut at its quietest point near the
middle and the halves alternate A, B, A, B with 600 ms of silence. In its
JSON only the times and speakers are a reference; each line's text is the
whole sentence marked with the half it belongs to.
