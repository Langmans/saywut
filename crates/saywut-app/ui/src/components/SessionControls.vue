<script setup lang="ts">
import { t } from "../i18n";

defineProps<{ running: boolean }>();
const emit = defineEmits<{ start: []; stop: [] }>();
</script>

<template>
  <span class="status" :class="{ live: running }">
    {{ running ? t("app-status-live") : t("app-status-stopped") }}
  </span>
  <button type="button" @click="running ? emit('stop') : emit('start')">
    {{ running ? t("app-stop") : t("app-start") }}
  </button>
</template>

<style scoped>
.status {
  font-size: 12px;
  color: var(--muted);
}

.status.live {
  color: var(--live);
}

.status.live::before {
  content: "● ";
}

button {
  font: inherit;
  font-size: 12px;
  padding: 3px 10px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: var(--bg);
  color: var(--text);
  cursor: pointer;
}

button:hover {
  border-color: var(--accent);
}
</style>
