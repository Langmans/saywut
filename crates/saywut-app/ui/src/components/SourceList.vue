<script setup lang="ts">
import type { SourceState } from "../session";
import LevelMeter from "./LevelMeter.vue";
import SourceChip from "./SourceChip.vue";

defineProps<{ sources: SourceState[] }>();
</script>

<template>
  <section v-if="sources.length" class="sources">
    <div v-for="s in sources" :key="s.source.id" class="source">
      <SourceChip :source="s.source" />
      <LevelMeter :dbfs="s.dbfs" />
    </div>
  </section>
</template>

<style scoped>
.sources {
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding: 8px 12px;
  background: var(--panel);
  border-bottom: 1px solid var(--border);
}

.source {
  display: flex;
  flex-direction: column;
  gap: 4px;
}
</style>
