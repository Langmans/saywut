<script setup lang="ts">
// A speaker's name in a colour of its own; ids beyond the palette reuse it.
const PALETTE_SIZE = 6;

defineProps<{ id: number; label: string }>();
</script>

<template>
  <span class="speaker" :class="`s${id % PALETTE_SIZE}`">{{ label }}</span>
</template>

<style scoped>
.speaker {
  display: block;
  font-size: 12px;
  font-weight: 600;
}

.s0 { color: var(--s0); }
.s1 { color: var(--s1); }
.s2 { color: var(--s2); }
.s3 { color: var(--s3); }
.s4 { color: var(--s4); }
.s5 { color: var(--s5); }
</style>
