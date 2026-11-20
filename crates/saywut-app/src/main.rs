// No console window next to the app in release builds on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Opens the window. This file is the one part of the app that coverage
//! cannot reach (see AGENTS.md); everything it calls is tested in `lib.rs`.
//!
//! Usage: `saywut-app [CLIP.wav]`, with the transcript in `CLIP.json`.
//! `SAYWUT_LANG` overrides the UI language (e.g. `nl`).

fn main() {
    let state = saywut_app::AppState::new(
        std::env::args_os().nth(1).map(Into::into),
        std::env::var("SAYWUT_LANG").ok(),
    );
    saywut_app::builder(tauri::Builder::default(), state)
        .run(tauri::generate_context!())
        .expect("the Saywut window failed to open");
}
