<script setup lang="ts">
import { computed } from "vue";
import type { InitializationSnapshot } from "../types";

const props = defineProps<{ initialization: InitializationSnapshot }>();
const latestEvent = computed(() => props.initialization.events[props.initialization.events.length - 1]);
const isWorking = computed(() => !["complete", "failed"].includes(props.initialization.phase));
</script>

<template>
  <section v-if="latestEvent" class="initialization-status-row" aria-live="polite" aria-label="Codex Pulse background refresh">
    <span class="initialization-status-row__indicator" :class="{ 'initialization-status-row__indicator--working': isWorking }" aria-hidden="true" />
    <span class="initialization-status-row__summary">{{ latestEvent.summary }}</span>
  </section>
</template>
