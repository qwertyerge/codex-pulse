import { nextTick, ref } from "vue";
import { describe, expect, it } from "vitest";

import { i18n, resolveLocale } from "../i18n";
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
  });
});
