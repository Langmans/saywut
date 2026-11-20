<script setup lang="ts">
import type { Line } from "../session";
import SpeakerLabel from "./SpeakerLabel.vue";

// `speakers` maps speaker ids to display labels; an unknown id shows as #id.
defineProps<{ line: Line; speakers: Record<string, string> }>();
</script>

<template>
  <p class="line" :class="{ partial: line.partial }">
    <SpeakerLabel
      v-if="line.speaker !== undefined"
      :id="line.speaker"
      :label="speakers[line.speaker] ?? `#${line.speaker}`"
    />
    <span class="text">{{ line.text }}</span>
  </p>
</template>

<style scoped>
.line {
  margin: 0 0 8px;
}

.partial .text {
  font-style: italic;
  color: var(--muted);
}
</style>
