<script setup lang="ts">
// A horizontal level meter over the top METER_RANGE_DB of the scale. The
// colour zones sit on the track; `.rest` covers the part above the level
// from the right, so the zones never stretch with the level.
import { computed } from "vue";
import { t } from "../i18n";

const METER_RANGE_DB = 60;

const props = defineProps<{ dbfs: number }>();

const shown = computed(() => Math.max(-METER_RANGE_DB, Math.min(0, props.dbfs)));
const restWidth = computed(() => `${(-shown.value / METER_RANGE_DB) * 100}%`);
const label = computed(() =>
  props.dbfs <= -METER_RANGE_DB ? "−∞" : `${Math.round(props.dbfs)} dB`,
);
</script>

<template>
  <div class="level">
    <div
      class="meter"
      role="meter"
      :aria-label="t('app-level')"
      :aria-valuemin="-METER_RANGE_DB"
      aria-valuemax="0"
      :aria-valuenow="Math.round(shown)"
    >
      <div class="rest" :style="{ width: restWidth }"></div>
    </div>
    <span class="db">{{ label }}</span>
  </div>
</template>

<style scoped>
.level {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 3.5em;
  gap: 8px;
  align-items: center;
}

/* Green to -18 dB, yellow to -6 dB, then red. */
.meter {
  position: relative;
  height: 8px;
  border-radius: 4px;
  overflow: hidden;
  background: linear-gradient(
    to right,
    var(--meter-low) 0 70%,
    var(--meter-mid) 70% 90%,
    var(--meter-high) 90%
  );
}

.rest {
  position: absolute;
  inset: 0 0 0 auto;
  background: var(--meter-bg);
  transition: width 60ms linear;
}

.db {
  text-align: right;
  font-size: 11px;
  font-variant-numeric: tabular-nums;
  color: var(--muted);
}
</style>
