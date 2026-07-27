<script setup lang="ts">
import { Languages, Monitor, Moon, Pin, PinOff, Sun } from "@lucide/vue";
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import type { LocaleMode, ThemeMode } from "../types";

const props = defineProps<{ activeCount: number; alwaysOnTop: boolean; theme: ThemeMode; locale: LocaleMode }>();
const emit = defineEmits<{ "toggle-pin": []; "set-theme": [theme: ThemeMode]; "set-locale": [locale: LocaleMode] }>();
const { t } = useI18n();
const localeMenuOpen = ref(false);
const locales: Array<{ value: LocaleMode; label: string }> = [
  { value: "system", label: "language.system" },
  { value: "zh-CN", label: "language.zhCn" },
  { value: "en", label: "language.en" },
  { value: "fr", label: "language.fr" },
  { value: "de", label: "language.de" }
];

function setLocale(locale: LocaleMode) {
  emit("set-locale", locale);
  localeMenuOpen.value = false;
}
</script>

<template>
  <header class="top-bar">
    <span class="top-bar__brand">
      <svg class="top-bar__mark" viewBox="0 0 20 20" aria-hidden="true">
        <path d="M2 11h4l2-5 3 9 2-4h5" />
      </svg>
      <span class="top-bar__name">Codex Pulse</span>
      <span class="top-bar__count">{{ t("topBar.active", { count: props.activeCount }) }}</span>
    </span>
    <span class="top-bar__controls">
      <span class="top-bar__theme-group" role="group" :aria-label="t('topBar.appearance')">
        <button type="button" :title="t('topBar.light')" :aria-label="t('topBar.light')" :aria-pressed="props.theme === 'light'" @click="emit('set-theme', 'light')">
          <Sun aria-hidden="true" />
        </button>
        <button type="button" :title="t('topBar.dark')" :aria-label="t('topBar.dark')" :aria-pressed="props.theme === 'dark'" @click="emit('set-theme', 'dark')">
          <Moon aria-hidden="true" />
        </button>
        <button type="button" :title="t('topBar.system')" :aria-label="t('topBar.system')" :aria-pressed="props.theme === 'system'" @click="emit('set-theme', 'system')">
          <Monitor aria-hidden="true" />
        </button>
      </span>
      <span class="top-bar__locale">
        <button type="button" :title="t('topBar.language')" :aria-label="t('topBar.language')" aria-haspopup="menu" :aria-expanded="localeMenuOpen" @click="localeMenuOpen = !localeMenuOpen">
          <Languages aria-hidden="true" />
        </button>
        <span v-if="localeMenuOpen" class="top-bar__locale-menu" role="menu" :aria-label="t('topBar.language')">
          <button
            v-for="option in locales"
            :key="option.value"
            type="button"
            role="menuitemradio"
            :data-locale="option.value"
            :aria-checked="props.locale === option.value"
            @click="setLocale(option.value)"
          >{{ t(option.label) }}</button>
        </span>
      </span>
      <button class="top-bar__pin" type="button" :title="props.alwaysOnTop ? t('topBar.unpin') : t('topBar.pin')" :aria-label="props.alwaysOnTop ? t('topBar.unpin') : t('topBar.pin')" @click="emit('toggle-pin')">
        <PinOff v-if="props.alwaysOnTop" aria-hidden="true" />
        <Pin v-else aria-hidden="true" />
      </button>
    </span>
  </header>
</template>
