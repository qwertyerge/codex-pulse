<script setup lang="ts">
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import EmptyState from "./components/EmptyState.vue";
import FooterStatus from "./components/FooterStatus.vue";
import InitializationStatusRow from "./components/InitializationStatusRow.vue";
import MonitoringBanner from "./components/MonitoringBanner.vue";
import SessionCard from "./components/SessionCard.vue";
import TopBar from "./components/TopBar.vue";
import { usePulse } from "./composables/usePulse";
import { useMonotonicClock } from "./composables/useMonotonicClock";
import { useTheme } from "./composables/useTheme";
import type { InitializationEvent } from "./types";

const pulse = usePulse();
const clock = useMonotonicClock();
let refresh: ReturnType<typeof setInterval> | undefined;
let unlisten: UnlistenFn | undefined;
let unlistenInitialization: UnlistenFn | undefined;
let backgroundInitializationHideTimer: ReturnType<typeof setTimeout> | undefined;
let displayedBackgroundInitializationRun = -1;
let terminalBackgroundInitializationRun = -1;
const theme = useTheme(computed(() => pulse.snapshot.value.theme));
const initialScreenFinished = ref(false);
const showBackgroundInitialization = ref(false);

watch(() => pulse.snapshot.value.initialization, (initialization) => {
  if (!initialization.events.length) return;
  const isTerminal = initialization.phase === "complete" || initialization.phase === "failed";
  if (initialization.runId === 1) {
    if (isTerminal) initialScreenFinished.value = true;
    return;
  }
  if (!initialScreenFinished.value) return;
  if (initialization.runId !== displayedBackgroundInitializationRun) {
    displayedBackgroundInitializationRun = initialization.runId;
    terminalBackgroundInitializationRun = -1;
    if (backgroundInitializationHideTimer) clearTimeout(backgroundInitializationHideTimer);
    showBackgroundInitialization.value = true;
  }
  if (showBackgroundInitialization.value && isTerminal && terminalBackgroundInitializationRun !== initialization.runId) {
    terminalBackgroundInitializationRun = initialization.runId;
    backgroundInitializationHideTimer = setTimeout(() => { showBackgroundInitialization.value = false; }, 2_200);
  }
}, { deep: true });

onMounted(async () => {
  await pulse.load();
  unlisten = await listen("sessions-changed", () => { void pulse.load(); });
  unlistenInitialization = await listen<InitializationEvent>("initialization-progress", (event) => {
    pulse.mergeInitializationEvent(event.payload);
  });
  theme.start();
  clock.start();
  refresh = setInterval(() => { void pulse.load(); }, 2_000);
});

onUnmounted(() => {
  clock.stop();
  if (refresh) clearInterval(refresh);
  if (backgroundInitializationHideTimer) clearTimeout(backgroundInitializationHideTimer);
  unlisten?.();
  unlistenInitialization?.();
  theme.stop();
});
</script>

<template>
  <main class="pulse-shell" :class="{ 'pulse-shell--background-refresh': showBackgroundInitialization }">
    <TopBar
      :active-count="pulse.snapshot.value.sessions.length"
      :always-on-top="pulse.snapshot.value.alwaysOnTop"
      :theme="pulse.snapshot.value.theme"
      @toggle-pin="pulse.togglePin"
      @set-theme="pulse.setTheme"
    />
    <MonitoringBanner
      :enabled="pulse.snapshot.value.monitoring.enabled"
      :needs-repair="pulse.snapshot.value.monitoring.needsRepair"
      :degraded-reason="pulse.snapshot.value.monitoring.degradedReason"
      @enable="pulse.enableMonitoring"
    />
    <p v-if="pulse.error.value" class="pulse-shell__error" role="status">{{ pulse.error.value }}</p>
    <TransitionGroup v-if="pulse.snapshot.value.sessions.length" name="task-card" tag="section" class="session-list" aria-label="Active Codex sessions">
      <SessionCard
        v-for="session in pulse.snapshot.value.sessions"
        :key="session.threadId"
        :session="session"
        :now-ms="clock.nowMs.value"
        @open="pulse.openThread"
      />
    </TransitionGroup>
    <EmptyState
      v-else
      :loading="pulse.snapshot.value.isLoading"
      :initialization="pulse.snapshot.value.initialization"
    />
    <div class="footer-stack">
      <InitializationStatusRow
        v-if="showBackgroundInitialization"
        :initialization="pulse.snapshot.value.initialization"
      />
      <FooterStatus
        :quota="pulse.snapshot.value.weeklyQuota"
        :now-ms="clock.nowMs.value"
        :active-session-count="pulse.snapshot.value.sessions.length"
      />
    </div>
  </main>
</template>
