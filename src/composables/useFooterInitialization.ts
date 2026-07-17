import { ref, watch, type Ref } from "vue";
import type { InitializationSnapshot } from "../types";

export const FOOTER_STATUS_THROTTLE_MS = 600;
export const FOOTER_STATUS_HIDE_MS = 2_200;

export function useFooterInitialization(source: Readonly<Ref<InitializationSnapshot>>) {
  const visible = ref(false);
  const initialization = ref<InitializationSnapshot>();
  let initialScreenFinished = false;
  let activeRunId = -1;
  let displayTimer: ReturnType<typeof setTimeout> | undefined;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;

  function clearDisplayTimer() {
    if (!displayTimer) return;
    clearTimeout(displayTimer);
    displayTimer = undefined;
  }

  function clearHideTimer() {
    if (!hideTimer) return;
    clearTimeout(hideTimer);
    hideTimer = undefined;
  }

  function show(snapshot: InitializationSnapshot) {
    initialization.value = snapshot;
    visible.value = true;
  }

  function scheduleStatus(snapshot: InitializationSnapshot, terminal: boolean) {
    clearDisplayTimer();
    clearHideTimer();
    visible.value = false;
    initialization.value = undefined;
    displayTimer = setTimeout(() => {
      show(snapshot);
      if (terminal) scheduleHide(snapshot.runId);
      displayTimer = undefined;
    }, FOOTER_STATUS_THROTTLE_MS);
  }

  function scheduleHide(runId: number) {
    clearHideTimer();
    hideTimer = setTimeout(() => {
      if (activeRunId === runId) {
        visible.value = false;
        initialization.value = undefined;
      }
      hideTimer = undefined;
    }, FOOTER_STATUS_HIDE_MS);
  }

  const stopWatching = watch(source, (snapshot) => {
    if (!snapshot.events.length) return;
    const terminal = snapshot.phase === "complete" || snapshot.phase === "failed";
    if (snapshot.runId === 1) {
      if (terminal) initialScreenFinished = true;
      return;
    }
    if (!initialScreenFinished) return;

    if (snapshot.runId !== activeRunId) {
      activeRunId = snapshot.runId;
      clearDisplayTimer();
      clearHideTimer();
      visible.value = false;
      initialization.value = undefined;
    }

    if (terminal) {
      scheduleStatus(snapshot, true);
      return;
    }

    scheduleStatus(snapshot, false);
  }, { deep: true, immediate: true });

  return {
    visible,
    initialization,
    stop() {
      stopWatching();
      clearDisplayTimer();
      clearHideTimer();
    }
  };
}
