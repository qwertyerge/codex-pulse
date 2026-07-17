<script setup lang="ts">
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { onMounted, onUnmounted } from "vue";
import EmptyState from "./components/EmptyState.vue";
import MonitoringBanner from "./components/MonitoringBanner.vue";
import SessionCard from "./components/SessionCard.vue";
import TopBar from "./components/TopBar.vue";
import { usePulse } from "./composables/usePulse";
import { useMonotonicClock } from "./composables/useMonotonicClock";

const pulse = usePulse();
const clock = useMonotonicClock();
let refresh: ReturnType<typeof setInterval> | undefined;
let unlisten: UnlistenFn | undefined;

onMounted(async () => {
  await pulse.load();
  unlisten = await listen("sessions-changed", () => { void pulse.load(); });
  clock.start();
  refresh = setInterval(() => { void pulse.load(); }, 2_000);
});

onUnmounted(() => {
  clock.stop();
  if (refresh) clearInterval(refresh);
  unlisten?.();
});
</script>

<template>
  <main class="pulse-shell">
    <TopBar
      :active-count="pulse.snapshot.value.sessions.length"
      :always-on-top="pulse.snapshot.value.alwaysOnTop"
      @toggle-pin="pulse.togglePin"
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
    <EmptyState v-else :loading="pulse.snapshot.value.isLoading" />
  </main>
</template>
