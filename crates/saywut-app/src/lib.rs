//! Saywut desktop app: a narrow panel with source chips, a level meter per
//! source and the live transcript.
//!
//! Rust owns the session and the strings; the frontend in `ui/` only
//! renders. A session runs on its own thread and sends every [`LiveEvent`]
//! to the window as a [`EVENT`] event. Until real capture exists, the
//! session is a [`FakeSession`] replaying a clip with its transcript JSON,
//! by default the committed `testdata/speech/two-voices`.
//!
//! Each start bumps a generation number. A session thread forwards events
//! only while its generation is current, so a session that was replaced
//! falls silent at once, even while its clock is still sleeping.

use std::{
    collections::BTreeMap,
    ops::ControlFlow,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, PoisonError},
    thread,
};

use saywut_capture_fake::{Clock, FakeError, FakeSession, RealClock};
use saywut_core::{live::LiveEvent, transcript::Speaker};
use saywut_i18n::Localizer;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime, State, WebviewWindow};

/// Name of the event that carries a [`LiveEvent`] to the frontend.
pub const EVENT: &str = "live";

/// Prefix of the Fluent messages the frontend shows.
const UI_PREFIX: &str = "app-";

/// The clip replayed when none is given: two Dutch voices, committed, so the
/// app starts on a fresh clone.
pub fn default_clip() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/speech/two-voices.wav")
}

type MakeClock = fn() -> Box<dyn Clock + Send>;

pub struct AppState {
    clip: PathBuf,
    locale: &'static str,
    clock: MakeClock,
    run: Arc<Mutex<Run>>,
}

#[derive(Default)]
struct Run {
    generation: u64,
    stop: bool,
}

impl AppState {
    /// `clip` defaults to [`default_clip`]. `lang` is a BCP-47 tag; without
    /// it the OS locale decides.
    pub fn new(clip: Option<PathBuf>, lang: Option<String>) -> Self {
        let lang = lang.or_else(sys_locale::get_locale).unwrap_or_default();
        Self {
            clip: clip.unwrap_or_else(default_clip),
            locale: saywut_i18n::negotiate(&lang),
            clock: real_clock,
            run: Arc::default(),
        }
    }

    fn localizer(&self) -> Localizer {
        Localizer::new(self.locale)
    }
}

fn real_clock() -> Box<dyn Clock + Send> {
    Box::new(RealClock::start())
}

/// Adds the state and the window's commands to `base`. `main` passes the
/// default builder and runs it; tests pass Tauri's mock builder.
pub fn builder<R: Runtime>(base: tauri::Builder<R>, state: AppState) -> tauri::Builder<R> {
    base.manage(state).invoke_handler(tauri::generate_handler![
        ui_strings,
        start,
        stop,
        set_always_on_top
    ])
}

#[derive(Serialize)]
struct UiStrings {
    lang: &'static str,
    messages: BTreeMap<String, String>,
}

/// Every `app-*` message of the UI locale, formatted without arguments.
#[tauri::command]
fn ui_strings(state: State<'_, AppState>) -> UiStrings {
    let loc = state.localizer();
    let messages = loc
        .ids()
        .filter(|id| id.starts_with(UI_PREFIX))
        .map(|id| (id.to_owned(), loc.message(id, &[])))
        .collect();
    UiStrings {
        lang: loc.tag(),
        messages,
    }
}

#[derive(Serialize)]
struct SessionInfo {
    /// Display label per speaker id.
    speakers: BTreeMap<u32, String>,
}

/// Starts a session on the clip, replacing any running one. The events
/// follow as [`EVENT`] events.
#[tauri::command]
fn start<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Result<SessionInfo, String> {
    let loc = state.localizer();
    let session = FakeSession::open(&state.clip).map_err(|e| open_error(&loc, &state.clip, &e))?;
    let info = SessionInfo {
        speakers: speaker_labels(&loc, session.speakers()),
    };
    let generation = {
        let mut run = lock(&state.run);
        run.generation += 1;
        run.stop = false;
        run.generation
    };
    let run = Arc::clone(&state.run);
    let mut clock = (state.clock)();
    thread::spawn(move || {
        session.run(clock.as_mut(), |event| {
            forward(&run, generation, event, |e| {
                let _ = app.emit(EVENT, e);
            })
        });
    });
    Ok(info)
}

/// Asks the running session to stop; it still sends its `stopped` event.
#[tauri::command]
fn stop(state: State<'_, AppState>) {
    lock(&state.run).stop = true;
}

#[tauri::command]
fn set_always_on_top<R: Runtime>(window: WebviewWindow<R>, on: bool) -> tauri::Result<()> {
    // tauri::Error serializes as its message, which the frontend shows.
    window.set_always_on_top(on)
}

fn lock(run: &Mutex<Run>) -> std::sync::MutexGuard<'_, Run> {
    // Run holds two plain values that are never left half-written.
    run.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Emits `event` of session `generation` unless a newer session started,
/// and breaks off once a stop was asked for.
fn forward(
    run: &Mutex<Run>,
    generation: u64,
    event: &LiveEvent,
    emit: impl FnOnce(&LiveEvent),
) -> ControlFlow<()> {
    let run = lock(run);
    if run.generation != generation {
        return ControlFlow::Break(());
    }
    emit(event);
    if run.stop {
        ControlFlow::Break(())
    } else {
        ControlFlow::Continue(())
    }
}

/// A named speaker by name, otherwise "Speaker n" with n counting from 1.
fn speaker_labels(loc: &Localizer, speakers: &[Speaker]) -> BTreeMap<u32, String> {
    speakers
        .iter()
        .map(|s| {
            let label = s.name.clone().unwrap_or_else(|| {
                let n = u64::from(s.id.0) + 1;
                loc.message("speaker-label", &[("n", n.into())])
            });
            (s.id.0, label)
        })
        .collect()
}

fn open_error(loc: &Localizer, wav: &Path, error: &FakeError) -> String {
    let (id, path, reason) = match error {
        FakeError::Wav(e) => ("error-read-audio", wav.to_path_buf(), e.to_string()),
        FakeError::Read(e) => (
            "error-read-script",
            wav.with_extension("json"),
            e.to_string(),
        ),
        FakeError::Transcript(e) => (
            "error-read-script",
            wav.with_extension("json"),
            e.to_string(),
        ),
    };
    loc.message(
        id,
        &[
            ("path", path.display().to_string().into()),
            ("reason", reason.into()),
        ],
    )
}

#[cfg(test)]
mod tests;
