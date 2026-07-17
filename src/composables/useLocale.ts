import { watch, type Ref } from "vue";
import { i18n, resolveLocale } from "../i18n";
import type { LocaleMode } from "../types";

export function useLocale(preference: Readonly<Ref<LocaleMode>>, getBrowserLanguage = () => navigator.language) {
  const apply = () => {
    i18n.global.locale.value = resolveLocale(preference.value, getBrowserLanguage());
  };
  const stopWatching = watch(preference, apply, { immediate: true });
  const onLanguageChange = () => {
    if (preference.value === "system") apply();
  };

  if (typeof window !== "undefined") window.addEventListener("languagechange", onLanguageChange);

  return () => {
    stopWatching();
    if (typeof window !== "undefined") window.removeEventListener("languagechange", onLanguageChange);
  };
}
