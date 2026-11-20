use super::*;
use saywut_capture_fake::NoWait;
use saywut_core::transcript::SpeakerId;
use serde_json::{Value, json};
use std::{sync::mpsc, time::Duration};
use tauri::{
    App, Listener, Manager, WebviewWindowBuilder,
    ipc::{CallbackFn, InvokeBody},
    test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets},
    webview::InvokeRequest,
};

// The app's own origin, which differs per platform; commands are only
// allowed from there. Constants rather than `cfg!`, so no branch goes
// unrun on any one platform.
#[cfg(windows)]
const APP_ORIGIN: &str = "http://tauri.localhost";
#[cfg(not(windows))]
const APP_ORIGIN: &str = "tauri://localhost";

fn state(clip: PathBuf, lang: &str) -> AppState {
    AppState {
        clock: || Box::new(NoWait),
        ..AppState::new(Some(clip), Some(lang.into()))
    }
}

fn open(state: AppState) -> (App<MockRuntime>, WebviewWindow<MockRuntime>) {
    let app = builder(mock_builder(), state)
        .build(mock_context(noop_assets()))
        .unwrap();
    let window = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    (app, window)
}

fn invoke(window: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
    let request = InvokeRequest {
        cmd: cmd.into(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url: APP_ORIGIN.parse().unwrap(),
        body: InvokeBody::Json(body),
        headers: Default::default(),
        invoke_key: INVOKE_KEY.to_string(),
    };
    get_ipc_response(window, request).map(|b| b.deserialize().unwrap())
}

/// A 200 ms clip at 1 kHz with two segments, one by an unnamed speaker.
fn clip(name: &str) -> PathBuf {
    let wav = std::env::temp_dir().join(format!("saywut-app-{}-{name}.wav", std::process::id()));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 1000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(&wav, spec).unwrap();
    for _ in 0..200 {
        w.write_sample(8192_i16).unwrap();
    }
    w.finalize().unwrap();
    let transcript = json!({
        "version": 1,
        "speakers": [{"id": 0, "name": "David"}],
        "segments": [
            {"start_ms": 0, "end_ms": 90, "speaker": 0, "text": "Good morning."},
            {"start_ms": 100, "end_ms": 190, "speaker": 1, "text": "Hi."}
        ]
    });
    std::fs::write(wav.with_extension("json"), transcript.to_string()).unwrap();
    wav
}

fn remove(wav: &Path) {
    let _ = std::fs::remove_file(wav);
    let _ = std::fs::remove_file(wav.with_extension("json"));
}

#[test]
fn default_clip_is_the_english_dialogue() {
    assert!(default_clip().ends_with("testdata/speech/two-voices.wav"));
    assert!(default_clip().is_file());
}

#[test]
fn sessions_run_on_the_wall_clock() {
    let mut clock = (AppState::new(None, Some("en".into())).clock)();
    let before = std::time::Instant::now();
    clock.wait_until(2);
    assert!(before.elapsed() >= Duration::from_millis(1));
}

#[test]
fn ui_language_is_negotiated() {
    assert_eq!(AppState::new(None, Some("de-CH".into())).locale, "de");
    assert_eq!(AppState::new(None, Some("xx".into())).locale, "en");
    // Without a tag the OS decides; whatever it is, it maps to a locale.
    assert!(saywut_i18n::available().contains(&AppState::new(None, None).locale));
}

#[test]
fn ui_strings_are_the_app_messages_of_the_locale() {
    let (_app, window) = open(state(default_clip(), "nl"));
    let strings = invoke(&window, "ui_strings", json!({})).unwrap();
    assert_eq!(strings["lang"], "nl");
    assert_eq!(strings["messages"]["app-status-stopped"], "Gestopt");
    let messages = strings["messages"].as_object().unwrap();
    assert!(messages.keys().all(|k| k.starts_with("app-")));
    // The frontend shows these as they are, so none may take arguments.
    assert!(
        messages
            .values()
            .all(|v| !v.as_str().unwrap().contains('{'))
    );
}

#[test]
fn start_replays_the_clip_as_events() {
    let wav = clip("start");
    let (app, window) = open(state(wav.clone(), "en"));
    let (tx, rx) = mpsc::channel();
    app.listen_any(EVENT, move |e| {
        let _ = tx.send(e.payload().to_owned());
    });
    let info = invoke(&window, "start", json!({})).unwrap();
    assert_eq!(info, json!({"speakers": {"0": "David", "1": "Speaker 2"}}));

    let mut kinds = Vec::new();
    loop {
        let payload = rx.recv_timeout(Duration::from_secs(10)).unwrap();
        let event: LiveEvent = serde_json::from_str(&payload).unwrap();
        let kind = to_kind(&event);
        kinds.push(kind);
        if kind == "stopped" {
            break;
        }
    }
    remove(&wav);
    assert_eq!(kinds.first(), Some(&"started"));
    assert_eq!(kinds.iter().filter(|k| **k == "final").count(), 2);
    assert_eq!(kinds.iter().filter(|k| **k == "level").count(), 4);
}

fn to_kind(event: &LiveEvent) -> &'static str {
    match event {
        LiveEvent::Started { .. } => "started",
        LiveEvent::Level { .. } => "level",
        LiveEvent::Partial { .. } => "partial",
        LiveEvent::Final { .. } => "final",
        LiveEvent::Stopped { .. } => "stopped",
    }
}

#[test]
fn start_reports_a_missing_clip_in_the_ui_language() {
    let (_app, window) = open(state(PathBuf::from("no/such/clip.wav"), "nl"));
    let err = invoke(&window, "start", json!({})).unwrap_err();
    let text = err.as_str().unwrap();
    assert!(text.starts_with("Kon audiobestand no"), "{text}");
}

#[test]
fn start_reports_a_missing_or_bad_transcript() {
    let wav = clip("bad");
    let (_app, window) = open(state(wav.clone(), "en"));
    std::fs::write(wav.with_extension("json"), "{").unwrap();
    let bad = invoke(&window, "start", json!({})).unwrap_err();
    std::fs::remove_file(wav.with_extension("json")).unwrap();
    let missing = invoke(&window, "start", json!({})).unwrap_err();
    remove(&wav);
    assert!(
        bad.as_str().unwrap().contains("invalid transcript JSON"),
        "{bad}"
    );
    assert!(
        missing
            .as_str()
            .unwrap()
            .starts_with("Could not read fake script"),
        "{missing}"
    );
}

#[test]
fn stop_asks_the_session_to_stop() {
    let (app, window) = open(state(default_clip(), "en"));
    invoke(&window, "stop", json!({})).unwrap();
    assert!(lock(&app.state::<AppState>().run).stop);
}

#[test]
fn always_on_top_can_be_toggled() {
    let (_app, window) = open(state(default_clip(), "en"));
    assert_eq!(
        invoke(&window, "set_always_on_top", json!({"on": true})),
        Ok(Value::Null)
    );
}

#[test]
fn forward_emits_while_current_and_breaks_on_stop() {
    let run = Mutex::new(Run {
        generation: 2,
        stop: false,
    });
    let event = LiveEvent::Stopped { at_ms: 0 };
    let sent = std::cell::Cell::new(0);
    let count = |_: &LiveEvent| sent.set(sent.get() + 1);
    assert!(forward(&run, 2, &event, count).is_continue());
    lock(&run).stop = true;
    assert!(forward(&run, 2, &event, count).is_break());
    assert_eq!(sent.get(), 2);
    // A replaced session falls silent.
    assert!(forward(&run, 1, &event, count).is_break());
    assert_eq!(sent.get(), 2);
}

#[test]
fn a_poisoned_lock_is_still_usable() {
    let run = Arc::new(Mutex::new(Run::default()));
    let held = Arc::clone(&run);
    let _ = thread::spawn(move || {
        let _guard = held.lock().unwrap();
        panic!("poison the lock");
    })
    .join();
    assert!(run.is_poisoned());
    assert_eq!(lock(&run).generation, 0);
}

#[test]
fn speaker_numbers_do_not_overflow() {
    let labels = speaker_labels(
        &Localizer::new("en"),
        &[Speaker {
            id: SpeakerId(u32::MAX),
            name: None,
        }],
    );
    assert_eq!(labels[&u32::MAX], "Speaker 4294967296");
}
