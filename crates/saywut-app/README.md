# saywut-app

The Saywut desktop app (Tauri 2): a narrow panel with source chips, a level
meter per source and the live transcript.

- **Rust owns the session and the strings**; the Vue frontend in `ui/` only
  renders. A session runs on its own thread and sends every `LiveEvent` to the
  window as a `live` event.
- **Until real capture exists**, the session is a `FakeSession` replaying a
  clip with its transcript JSON, by default the committed
  `testdata/speech/two-voices`.
- **Restarting is safe**: each start bumps a generation number, and a session
  thread forwards events only while its generation is current. A session that
  was replaced falls silent at once, even while its clock is still sleeping.

Running it: see `AGENTS.md` (Layout, `crates/saywut-app`).
