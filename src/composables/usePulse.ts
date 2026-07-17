import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { AppSnapshot } from "../types";

const emptySnapshot: AppSnapshot = {
  sessions: [],
  isLoading: true,
  monitoring: { enabled: false, needsRepair: false, staleCount: 0 },
  alwaysOnTop: false,
  launchAtLogin: false,
  locale: "system"
};

export function usePulse() {
  const snapshot = ref<AppSnapshot>(emptySnapshot);
  const error = ref<string>();

  async function load() {
    try {
      error.value = undefined;
      snapshot.value = await invoke<AppSnapshot>("get_snapshot");
    } catch (reason) {
      error.value = reason instanceof Error ? reason.message : String(reason);
    }
  }

  async function togglePin() {
    const previous = snapshot.value;
    const value = !previous.alwaysOnTop;
    snapshot.value = { ...previous, alwaysOnTop: value };
    try {
      error.value = undefined;
      await invoke<boolean>("set_always_on_top", { value });
    } catch (reason) {
      snapshot.value = previous;
      error.value = reason instanceof Error ? reason.message : String(reason);
    }
  }

  async function openThread(threadId: string) {
    try {
      error.value = undefined;
      await invoke("open_thread", { threadId });
    } catch (reason) {
      error.value = reason instanceof Error ? reason.message : String(reason);
    }
  }

  async function enableMonitoring() {
    error.value = undefined;
    try {
      await invoke("enable_monitoring");
      await load();
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : String(cause);
    }
  }

  return { snapshot, error, load, togglePin, openThread, enableMonitoring };
}
