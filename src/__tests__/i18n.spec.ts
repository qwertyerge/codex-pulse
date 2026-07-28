import { nextTick, ref } from "vue";
import { describe, expect, it } from "vitest";

import { i18n, messages, resolveLocale } from "../i18n";
import { useLocale } from "../composables/useLocale";
import type { LocaleMode } from "../types";

describe("localization runtime", () => {
  it.each([
    ["system", "zh-Hans-CN", "zh-CN"],
    ["system", "fr-CA", "fr"],
    ["system", "de-AT", "de"],
    ["system", "ja-JP", "en"],
    ["de", "zh-CN", "de"]
  ] as const)("resolves %s with %s to %s", (preference, browserLanguage, expected) => {
    expect(resolveLocale(preference, browserLanguage)).toBe(expected);
  });

  it("updates the active locale when the saved preference changes", async () => {
    const preference = ref<LocaleMode>("system");
    const stop = useLocale(preference, () => "fr-CA");

    expect(i18n.global.locale.value).toBe("fr");

    preference.value = "de";
    await nextTick();

    expect(i18n.global.locale.value).toBe("de");
    stop();
    i18n.global.locale.value = "en";
  });

  it.each([
    ["en", ["No branch", "Default branch", "Remote repository", "Not configured"]],
    ["zh-CN", ["无分支", "默认分支", "远程仓库", "未配置"]],
    ["fr", ["Aucune branche", "Branche par défaut", "Dépôt distant", "Non configuré"]],
    ["de", ["Kein Branch", "Standardbranch", "Remote-Repository", "Nicht konfiguriert"]]
  ] as const)("defines complete Git copy for %s", (locale, expected) => {
    i18n.global.locale.value = locale;
    const actual = [
      i18n.global.t("session.noBranch"),
      i18n.global.t("session.defaultBranch"),
      i18n.global.t("session.remoteRepository"),
      i18n.global.t("session.notConfigured")
    ];
    i18n.global.locale.value = "en";
    expect(actual).toEqual(expected);
  });

  it.each(["zh-CN", "fr", "de"] as const)(
    "keeps updater keys complete and non-empty for %s",
    (locale) => {
      const englishKeys = Object.keys(messages.en.updater).sort();
      const localized = messages[locale].updater;

      expect(Object.keys(localized).sort()).toEqual(englishKeys);
      expect(
        Object.values(localized).every(
          (value) => typeof value === "string" && value.trim().length > 0
        )
      ).toBe(true);
    }
  );

  it("keeps the English updater keys non-empty", () => {
    expect(
      Object.values(messages.en.updater).every(
        (value) => value.trim().length > 0
      )
    ).toBe(true);
  });
});
