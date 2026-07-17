<script setup lang="ts">
defineProps<{ activeCount: number; alwaysOnTop: boolean }>();
defineEmits<{ "toggle-pin": [] }>();
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
      <button type="button" :aria-label="alwaysOnTop ? 'Unpin window' : 'Pin window to top'" @click="$emit('toggle-pin')">
        {{ alwaysOnTop ? "Unpin" : "Pin to Top" }}
      </button>
    </span>
  </header>
</template>
