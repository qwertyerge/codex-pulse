import { watch, type Ref } from "vue";
import type { ThemeMode } from "../types";

export function useTheme(theme: Ref<ThemeMode>) {
  const media = typeof window === "undefined" || !window.matchMedia
    ? undefined
    : window.matchMedia("(prefers-color-scheme: dark)");

  function apply() {
    const resolved = theme.value === "system"
      ? media?.matches ? "dark" : "light"
      : theme.value;
    document.documentElement.dataset.theme = resolved;
  }

  function onSystemThemeChange() {
    if (theme.value === "system") apply();
  }

  const stopWatching = watch(theme, apply, { immediate: true });

  function start() {
    media?.addEventListener("change", onSystemThemeChange);
    apply();
  }

  function stop() {
    media?.removeEventListener("change", onSystemThemeChange);
    stopWatching();
  }

  return { start, stop };
}
