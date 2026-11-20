# saywut-core

The platform-independent core: the transcript model, the engine ports
(`AsrEngine`, `Diarizer`, `SpeakerEmbedder`) with fakes, and all pure logic.
No OS access beyond parsing and formatting.

## Live events (`live`)

What a running session reports to the UI. A session sends `Started` first and
`Stopped` last; in between it sends levels, partial lines and final lines per
source. Each source has at most one partial line at a time: a new `Partial`
replaces the previous one for that source, and a `Final` replaces it with the
finished segment. Events serialize as JSON objects tagged by `"kind"`, the
shape the frontend receives.

## Voice profiles (`profiles`)

One voice embedding per named person, to recognize them in later recordings.
An embedding is a vector a speaker-embedding model computes from a stretch of
speech; two stretches of the same voice give vectors pointing the same way
(high cosine similarity). No audio is stored. Embeddings are still biometric
data: profiles are created only on the user's request, stay in a local file,
and can be deleted per name.
