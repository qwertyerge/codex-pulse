import { ref, watch, type Ref } from "vue";
import type { InitializationSnapshot } from "../types";

export const FOOTER_STATUS_THROTTLE_MS = 2_000;
export const FOOTER_STATUS_HIDE_MS = 2_000;

export function useFooterInitialization(source: Readonly<Ref<InitializationSnapshot>>) {
  const visible = ref(false);
  const initialization = ref<InitializationSnapshot>();
  let initialScreenFinished = false;
  let displayTimer: ReturnType<typeof setTimeout> | undefined;
  let hideTimer: ReturnType<typeof setTimeout> | undefined;
  let displayEpoch = 0;
  let hideEpoch = 0;

  function clearDisplayTimer() {
    displayEpoch += 1;
    if (!displayTimer) return;
    clearTimeout(displayTimer);
    displayTimer = undefined;
  }

  function clearHideTimer() {
    hideEpoch += 1;
    if (!hideTimer) return;
    clearTimeout(hideTimer);
    hideTimer = undefined;
  }

  function isTerminal(snapshot: InitializationSnapshot) {
    return snapshot.phase === "complete" || snapshot.phase === "failed";
  }

  function scheduleStatus(snapshot: InitializationSnapshot) {
    clearDisplayTimer();
    const epoch = displayEpoch;
    displayTimer = setTimeout(() => {
      if (epoch !== displayEpoch) return;
      displayTimer = undefined;
      initialization.value = snapshot;
      visible.value = true;
      if (isTerminal(snapshot)) scheduleHide();
    }, FOOTER_STATUS_THROTTLE_MS);
  }

  function scheduleHide() {
    clearHideTimer();
    const epoch = hideEpoch;
    hideTimer = setTimeout(() => {
      if (epoch !== hideEpoch) return;
      hideTimer = undefined;
      visible.value = false;
      initialization.value = undefined;
    }, FOOTER_STATUS_HIDE_MS);
  }

  const stopWatching = watch(source, (snapshot) => {
    if (!snapshot.events.length) return;
    if (snapshot.runId === 1) {
      if (isTerminal(snapshot)) initialScreenFinished = true;
      return;
    }
    if (!initialScreenFinished) return;

    if (visible.value) {
      clearDisplayTimer();
      clearHideTimer();
      initialization.value = snapshot;
      if (isTerminal(snapshot)) scheduleHide();
      return;
    }

    scheduleStatus(snapshot);
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
