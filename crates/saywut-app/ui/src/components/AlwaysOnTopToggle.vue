<script setup lang="ts">
// Keeps the window above others, so the panel stays visible next to a
// browser. If the window refuses, the box flips back and the error is passed on.
import { ref } from "vue";
import { setAlwaysOnTop } from "../backend";
import { t } from "../i18n";

const emit = defineEmits<{ error: [message: string] }>();

const on = ref(false);

async function toggle(): Promise<void> {
  try {
    await setAlwaysOnTop(on.value);
  } catch (err) {
    on.value = !on.value;
    emit("error", String(err));
  }
}
</script>

<template>
  <label class="toggle">
    <input v-model="on" type="checkbox" @change="toggle" />
    <span>{{ t("app-always-on-top") }}</span>
  </label>
</template>

<style scoped>
.toggle {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--muted);
  cursor: pointer;
}
</style>
