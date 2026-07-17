<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { initializationLabel } from "../lib/initializationLabel";
import type { InitializationSnapshot } from "../types";

const props = defineProps<{ initialization: InitializationSnapshot }>();
const { t } = useI18n();
const visibleEvents = ref<typeof props.initialization.events>([]);
const sourceEvents = computed(() => props.initialization.events.slice(-6));
const isWorking = computed(() => !["complete", "failed"].includes(props.initialization.phase));
let activeRunId = -1;
let scheduled: ReturnType<typeof setTimeout>[] = [];
let scheduledKeys = new Set<string>();

function clearPlayback() {
  scheduled.forEach(clearTimeout);
  scheduled = [];
  scheduledKeys.clear();
}

watch(sourceEvents, (events) => {
  if (props.initialization.runId !== activeRunId) {
    activeRunId = props.initialization.runId;
    clearPlayback();
    visibleEvents.value = [];
  }
  const visible = new Set(visibleEvents.value.map((event) => event.sequence));
  events.forEach((event, index) => {
    const key = `${event.runId}:${event.sequence}`;
    if (visible.has(event.sequence) || scheduledKeys.has(key)) return;
    scheduledKeys.add(key);
    const timeout = setTimeout(() => {
      visibleEvents.value = [...visibleEvents.value, event].slice(-6);
      scheduledKeys.delete(key);
    }, index * 140);
    scheduled.push(timeout);
  });
}, { immediate: true });

onBeforeUnmount(clearPlayback);
</script>

<template>
  <section class="initialization-feed" role="log" :aria-label="t('initialization.feedAria')" aria-live="polite">
    <TransitionGroup name="initialization-event" tag="ol" class="initialization-feed__list">
      <li
        v-for="(event, index) in visibleEvents"
        :key="`${event.runId}:${event.sequence}`"
        class="initialization-feed__event"
        :data-phase="event.phase"
      >
        <span class="initialization-feed__prefix" aria-hidden="true">›</span>
        <span class="initialization-feed__summary">{{ initializationLabel(t, event) }}</span>
        <template v-if="isWorking && index === visibleEvents.length - 1">
          <span class="initialization-feed__ellipsis" aria-hidden="true">......</span>
        </template>
      </li>
    </TransitionGroup>
  </section>
</template>
