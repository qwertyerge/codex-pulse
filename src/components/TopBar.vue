<script setup lang="ts">
import { Monitor, Moon, Pin, PinOff, Sun } from "@lucide/vue";
import type { ThemeMode } from "../types";

defineProps<{ activeCount: number; alwaysOnTop: boolean; theme: ThemeMode }>();
defineEmits<{ "toggle-pin": []; "set-theme": [theme: ThemeMode] }>();
</script>

<template>
  <header class="top-bar">
    <span class="top-bar__brand">
      <svg class="top-bar__mark" viewBox="0 0 20 20" aria-hidden="true">
        <path d="M2 11h4l2-5 3 9 2-4h5" />
      </svg>
      <span>Codex Pulse</span>
      <span class="top-bar__count">{{ activeCount }} active</span>
    </span>
    <span class="top-bar__controls">
      <span class="top-bar__theme-group" role="group" aria-label="Appearance">
        <button type="button" title="Use light appearance" aria-label="Use light appearance" :aria-pressed="theme === 'light'" @click="$emit('set-theme', 'light')">
          <Sun aria-hidden="true" />
        </button>
        <button type="button" title="Use dark appearance" aria-label="Use dark appearance" :aria-pressed="theme === 'dark'" @click="$emit('set-theme', 'dark')">
          <Moon aria-hidden="true" />
        </button>
        <button type="button" title="Follow system appearance" aria-label="Follow system appearance" :aria-pressed="theme === 'system'" @click="$emit('set-theme', 'system')">
          <Monitor aria-hidden="true" />
        </button>
      </span>
      <button class="top-bar__pin" type="button" :title="alwaysOnTop ? 'Unpin window' : 'Pin window to top'" :aria-label="alwaysOnTop ? 'Unpin window' : 'Pin window to top'" @click="$emit('toggle-pin')">
        <PinOff v-if="alwaysOnTop" aria-hidden="true" />
        <Pin v-else aria-hidden="true" />
      </button>
    </span>
  </header>
</template>
