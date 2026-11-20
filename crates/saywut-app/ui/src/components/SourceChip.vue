<script setup lang="ts">
import { computed } from "vue";
import { t } from "../i18n";
import type { Source, SourceKind } from "../live";

const props = defineProps<{ source: Source }>();

const kindLabel: Record<SourceKind, () => string> = {
  microphone: () => t("app-source-microphone"),
  loopback: () => t("app-source-loopback"),
  application: () => t("app-source-application"),
  file: () => t("app-source-file"),
};

const kind = computed(() => kindLabel[props.source.kind]());
</script>

<template>
  <span class="chip" :class="source.kind" :title="source.name">
    <span class="kind">{{ kind }}</span> · {{ source.name }}
  </span>
</template>

<style scoped>
.chip {
  display: inline-block;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  padding: 1px 8px;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--bg);
}

.kind {
  font-weight: 600;
}
</style>
