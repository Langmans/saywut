// The Rust commands and the live event, typed. Components talk to the
// backend only through this module, so specs mock it in one place.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { LiveEvent, SessionInfo, UiStrings } from "./live";

/** Name of the event that carries a LiveEvent (EVENT in lib.rs). */
export const LIVE_EVENT = "live";

export const uiStrings = () => invoke<UiStrings>("ui_strings");
export const startSession = () => invoke<SessionInfo>("start");
export const stopSession = () => invoke<void>("stop");
export const setAlwaysOnTop = (on: boolean) => invoke<void>("set_always_on_top", { on });

export const onLiveEvent = (handler: (event: LiveEvent) => void): Promise<UnlistenFn> =>
  listen<LiveEvent>(LIVE_EVENT, (e) => handler(e.payload));
