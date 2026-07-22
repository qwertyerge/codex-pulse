import { invoke } from "@tauri-apps/api/core";
import { ref } from "vue";
import type { AppSnapshot, InitializationEvent, LocaleMode, ThemeMode } from "../types";

const emptySnapshot: AppSnapshot = {
  sessions: [],
  weeklyQuota: undefined,
  isLoading: true,
  initialization: { runId: 0, phase: "idle", events: [] },
  monitoring: { enabled: false, needsRepair: false, staleCount: 0 },
  alwaysOnTop: false,
  launchAtLogin: false,
  locale: "system",
  theme: "system"
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

  async function openProjectPath(path: string) {
    try {
      error.value = undefined;
      await invoke("open_project_path", { path });
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

  function mergeInitializationEvent(event: InitializationEvent) {
    const previous = snapshot.value;
    const previousRunId = previous.initialization.runId;
    if (event.runId < previousRunId) return;
    const existing = event.runId > previousRunId ? [] : previous.initialization.events;
    if (existing.some((candidate) => candidate.runId === event.runId && candidate.sequence === event.sequence)) return;
    const events = [...existing, event]
      .sort((left, right) => left.sequence - right.sequence)
      .slice(-120);
    snapshot.value = {
      ...previous,
      initialization: { runId: event.runId, phase: event.phase, events }
    };
  }

  async function setTheme(theme: ThemeMode) {
    const previous = snapshot.value;
    snapshot.value = { ...previous, theme };
    try {
      error.value = undefined;
      const saved = await invoke<ThemeMode>("set_theme", { theme });
      snapshot.value = { ...snapshot.value, theme: saved };
    } catch (reason) {
      snapshot.value = previous;
      error.value = reason instanceof Error ? reason.message : String(reason);
    }
  }

  async function setLocale(locale: LocaleMode) {
    const previous = snapshot.value;
    snapshot.value = { ...previous, locale };
    try {
      error.value = undefined;
      const saved = await invoke<LocaleMode>("set_locale", { locale });
      snapshot.value = { ...snapshot.value, locale: saved };
    } catch (reason) {
      snapshot.value = previous;
      error.value = reason instanceof Error ? reason.message : String(reason);
    }
  }

  return { snapshot, error, load, togglePin, openThread, openProjectPath, enableMonitoring, mergeInitializationEvent, setTheme, setLocale };
}
