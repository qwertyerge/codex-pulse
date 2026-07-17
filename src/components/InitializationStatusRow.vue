<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { initializationLabel } from "../lib/initializationLabel";
import type { InitializationSnapshot } from "../types";

const props = defineProps<{ initialization: InitializationSnapshot }>();
const { t } = useI18n();
const latestEvent = computed(() => props.initialization.events[props.initialization.events.length - 1]);
const isWorking = computed(() => !["complete", "failed"].includes(props.initialization.phase));
</script>

<template>
  <section v-if="latestEvent" class="initialization-status-row" aria-live="polite" :aria-label="t('initialization.backgroundAria')">
    <span class="initialization-status-row__indicator" :class="{ 'initialization-status-row__indicator--working': isWorking }" aria-hidden="true" />
    <span class="initialization-status-row__summary">{{ initializationLabel(t, latestEvent) }}</span>
  </section>
</template>
