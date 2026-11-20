# saywut-capture-fake

A live session replayed from a WAV file and its reference transcript (same
name, `.json`), so the UI can be built and tested without a microphone or a
speech model.

The replay is computed up front as a timeline of `LiveEvent`s:

- a level every `FRAME_MS` (50 ms) of audio, measured over that frame;
- for each segment, a partial line that grows one word at a time between its
  start and end, then the final line at `end_ms`;
- `Started` first and `Stopped` once both audio and transcript are over.

`FakeSession::run` plays the timeline against a `Clock`: `RealClock` waits in
real time, `NoWait` does not wait at all, which is what the tests use.
