// UI strings: the app-* Fluent messages of the UI locale, formatted by the
// Rust side. A missing message shows its key, so the gap is visible.

import { ref } from "vue";
import type { UiStrings } from "./live";

const messages = ref<Record<string, string>>({});

export function setStrings(strings: UiStrings): void {
  messages.value = strings.messages;
  document.documentElement.lang = strings.lang;
}

export function t(key: string): string {
  return messages.value[key] ?? key;
}
