<script setup lang="ts">
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { computed, onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import EmptyState from "./components/EmptyState.vue";
import FooterStatus from "./components/FooterStatus.vue";
import InitializationStatusRow from "./components/InitializationStatusRow.vue";
import MonitoringBanner from "./components/MonitoringBanner.vue";
import SessionCard from "./components/SessionCard.vue";
import TopBar from "./components/TopBar.vue";
import { usePulse } from "./composables/usePulse";
import { useMonotonicClock } from "./composables/useMonotonicClock";
import { useTheme } from "./composables/useTheme";
import { useLocale } from "./composables/useLocale";
import { useFooterInitialization } from "./composables/useFooterInitialization";
import { useUpdater } from "./composables/useUpdater";
import type { InitializationEvent } from "./types";

const pulse = usePulse();
const { t } = useI18n();
const clock = useMonotonicClock();
const updater = useUpdater();
const updaterState = updater.state;
let refresh: ReturnType<typeof setInterval> | undefined;
let unlisten: UnlistenFn | undefined;
let unlistenInitialization: UnlistenFn | undefined;
const theme = useTheme(computed(() => pulse.snapshot.value.theme));
const stopLocale = useLocale(computed(() => pulse.snapshot.value.locale));
const {
  visible: showBackgroundInitialization,
  initialization: footerInitialization,
  stop: stopFooterInitialization
} = useFooterInitialization(computed(() => pulse.snapshot.value.initialization));

async function activateUpdate() {
  const version =
    updaterState.value.phase === "ready" ? updaterState.value.version : "";
  await updater.activate({
    title: t("updater.confirmTitle"),
    message: t("updater.confirmMessage", { version })
  });
}

onMounted(async () => {
  await pulse.load();
  updater.start();
  unlisten = await listen("sessions-changed", () => { void pulse.load(); });
  unlistenInitialization = await listen<InitializationEvent>("initialization-progress", (event) => {
    pulse.mergeInitializationEvent(event.payload);
  });
  theme.start();
  clock.start();
  refresh = setInterval(() => { void pulse.load(); }, 60_000);
});

onUnmounted(() => {
  updater.stop();
  clock.stop();
  if (refresh) clearInterval(refresh);
  unlisten?.();
  unlistenInitialization?.();
  theme.stop();
  stopLocale();
  stopFooterInitialization();
});
</script>

<template>
  <main class="pulse-shell" :class="{ 'pulse-shell--background-refresh': showBackgroundInitialization }">
    <TopBar
      :active-count="pulse.snapshot.value.sessions.length"
      :always-on-top="pulse.snapshot.value.alwaysOnTop"
      :theme="pulse.snapshot.value.theme"
      :locale="pulse.snapshot.value.locale"
      :update-state="updaterState"
      @toggle-pin="pulse.togglePin"
      @set-theme="pulse.setTheme"
      @set-locale="pulse.setLocale"
      @activate-update="activateUpdate"
    />
    <MonitoringBanner
      :enabled="pulse.snapshot.value.monitoring.enabled"
      :needs-repair="pulse.snapshot.value.monitoring.needsRepair"
      :degraded-reason="pulse.snapshot.value.monitoring.degradedReason"
      @enable="pulse.enableMonitoring"
    />
    <p v-if="pulse.error.value" class="pulse-shell__error" role="status">{{ pulse.error.value }}</p>
    <TransitionGroup v-if="pulse.snapshot.value.sessions.length" name="task-card" tag="section" class="session-list" :aria-label="t('empty.emptyLabel')">
      <SessionCard
        v-for="session in pulse.snapshot.value.sessions"
        :key="session.threadId"
        :session="session"
        :now-ms="clock.nowMs.value"
        @open="pulse.openThread"
        @open-project="pulse.openProjectPath"
      />
      <span key="session-list-end" class="session-list__end" aria-hidden="true">{{ t("list.end") }}</span>
    </TransitionGroup>
    <EmptyState
      v-else
      :loading="pulse.snapshot.value.isLoading"
      :initialization="pulse.snapshot.value.initialization"
    />
    <div class="footer-stack" :class="{ 'footer-stack--with-event': showBackgroundInitialization }">
      <Transition name="footer-status">
        <InitializationStatusRow
          v-if="showBackgroundInitialization && footerInitialization"
          :initialization="footerInitialization"
        />
      </Transition>
      <FooterStatus
        :quota="pulse.snapshot.value.weeklyQuota"
        :now-ms="clock.nowMs.value"
        :active-session-count="pulse.snapshot.value.sessions.length"
      />
    </div>
  </main>
</template>
