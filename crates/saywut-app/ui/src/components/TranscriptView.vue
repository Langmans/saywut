<script setup lang="ts">
// The scrolling transcript. It follows new lines while the reader is at the
// bottom and stays put once they scroll up to read.
import { nextTick, ref, watch } from "vue";
import { t } from "../i18n";
import type { Line } from "../session";
import TranscriptLine from "./TranscriptLine.vue";

/** How close to the bottom still counts as following, in pixels. */
const FOLLOW_SLACK_PX = 40;

const props = defineProps<{ lines: Line[]; speakers: Record<string, string> }>();

const box = ref<HTMLElement>();

// A new line or a growing last line; cheap, so a long transcript does not
// slow down every event.
watch(
  () => [props.lines.length, props.lines.at(-1)?.text],
  async () => {
    const el = box.value!;
    const following = el.scrollHeight - el.scrollTop - el.clientHeight < FOLLOW_SLACK_PX;
    await nextTick();
    if (following) el.scrollTop = el.scrollHeight;
  },
);
</script>

<template>
  <main ref="box" class="transcript" aria-live="polite">
    <p v-if="!lines.length" class="empty">{{ t("app-transcript-empty") }}</p>
    <TranscriptLine v-for="line in lines" :key="line.key" :line="line" :speakers="speakers" />
  </main>
</template>

<style scoped>
.transcript {
  flex: 1;
  overflow-y: auto;
  padding: 8px 12px 16px;
}

.empty {
  color: var(--muted);
  font-style: italic;
}
</style>
