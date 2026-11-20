<script setup lang="ts">
// Wires the backend to the components: loads the strings, listens for live
// events and starts a session as soon as the window is up.
import { onMounted, reactive, ref } from "vue";
import { onLiveEvent, startSession, stopSession, uiStrings } from "./backend";
import { setStrings, t } from "./i18n";
import { applyEvent, emptySession } from "./session";
import AlwaysOnTopToggle from "./components/AlwaysOnTopToggle.vue";
import SessionControls from "./components/SessionControls.vue";
import SourceList from "./components/SourceList.vue";
import TranscriptView from "./components/TranscriptView.vue";

const session = reactive(emptySession());
const speakers = ref<Record<string, string>>({});
const error = ref("");

async function start(): Promise<void> {
  error.value = "";
  try {
    speakers.value = (await startSession()).speakers;
  } catch (err) {
    error.value = String(err);
  }
}

async function stop(): Promise<void> {
  await stopSession();
}

onMounted(async () => {
  setStrings(await uiStrings());
  // Listen before starting, so the first events are not missed.
  await onLiveEvent((event) => applyEvent(session, event));
  await start();
});
</script>

<template>
  <header class="bar">
    <h1>{{ t("app-title") }}</h1>
    <SessionControls :running="session.running" @start="start" @stop="stop" />
    <AlwaysOnTopToggle class="on-top" @error="error = $event" />
  </header>
  <SourceList :sources="session.sources" />
  <TranscriptView :lines="session.lines" :speakers="speakers" />
  <p v-if="error" class="error" role="alert">{{ error }}</p>
</template>

<style scoped>
.bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 12px;
  background: var(--panel);
  border-bottom: 1px solid var(--border);
}

h1 {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}

.on-top {
  margin-left: auto;
}

.error {
  margin: 0;
  padding: 8px 12px;
  color: var(--error);
  border-top: 1px solid var(--border);
  background: var(--panel);
}
</style>
