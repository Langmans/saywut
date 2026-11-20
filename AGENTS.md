# Agent instructions

For any coding agent working in this repository. [PLAN.md](PLAN.md) has the
design and phases; [README.md](README.md) has build requirements.

## Layout

- `crates/saywut-core`: pure logic and the ports (traits). No OS or I/O
  beyond what is needed to parse and format.
- `crates/saywut-i18n`: Fluent localization; strings live in `locales/<tag>/`.
- `crates/capture-*`, engine and diarizer crates: thin adapters behind the
  core traits.
- `crates/saywut-cli`: the `saywut` binary. Keep `main.rs` a one-liner and
  put logic in `lib.rs` so it can be tested.
- `tools/`: helper scripts (e.g. `tts-clips.ps1` for synthetic test audio).

## Checks

All of these must pass before a commit; CI runs the same on Windows, macOS
and Linux:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo llvm-cov --workspace --fail-under-lines 100
```

## Rules

- **Spec first.** Write the failing test, then the code.
- **100% line coverage.** The only exception is the single call that opens
  an OS device; such paths go on the manual checklist in PLAN.md.
- **Languages are data.** A UI language is a folder under `locales/`; a
  speech language comes from what the engine reports. Never hard-code a
  language list in the core. Every key exists in every locale (a test
  enforces this).
- **No UI strings in the core.** The core takes labels from the caller.
- **Malformed input never panics.** Audio files, transcript JSON and
  anything from disk or the network return an error.
- **Models are data.** A new model or model version is an edit to
  `models.toml`: a revision-pinned URL (never `main`), sha256, size, license,
  languages and WER/DER budgets. Never commit model files and never download
  one without the user's consent. Before such a change is merged or
  released, the regression clips must pass their budgets on every model in
  the manifest; put the WER/DER before and after in the commit message. See
  "Models" in PLAN.md.
- **Comments explain how and why**, not history. What changed and why goes
  in the commit message or a note next to the code, not in source.
- **English in the repo**: code, comments, docs, commit messages.

## Before every commit: review

Review the staged diff in four passes and fix what they turn up, or name it
in the commit message if it is deliberately left:

1. **DRY**: duplicated logic or data that should be one function, constant
   or file. Do not merge things that only look alike.
2. **KISS**: the simplest thing that works. No abstraction, option or
   generic without a second user; no speculative configuration.
3. **Code**: correctness, edge cases (empty, zero, huge, overflow,
   off-by-one), error paths, naming, and whether the tests prove the
   behavior rather than the implementation.
4. **Security**: untrusted input (files, JSON, paths, model files, network)
   is validated and cannot panic, overflow or exhaust memory without bound;
   no secrets in code or logs; no network access unless the user enabled
   it; voice profiles are biometric data and stay local, opt-in and
   deletable; new dependencies are maintained and needed.

## Git

- Conventional Commits (`feat(cli): …`, `fix(core): …`, `chore: …`).
- One commit per finished unit with all checks green.
- Bigger or risky work on a branch; small changes straight on `main`.
- Do not push unless asked.
