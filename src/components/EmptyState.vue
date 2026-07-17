<script setup lang="ts">
import InitializationFeed from "./InitializationFeed.vue";
import type { InitializationSnapshot } from "../types";

defineProps<{ loading: boolean; initialization: InitializationSnapshot }>();
</script>

<template>
  <section class="empty-state" :aria-label="loading ? 'Loading Codex sessions' : 'No active Codex sessions'">
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M3 13h5l2-5 4 10 2-5h5" />
    </svg>
    <strong>{{ loading ? "Loading active Codex sessions" : "No active Codex sessions" }}</strong>
    <span>{{ loading ? "Reconciling recent Codex activity…" : "Waiting for a running Codex task." }}</span>
    <InitializationFeed v-if="loading && initialization.runId === 1 && initialization.events.length" :initialization="initialization" />
  </section>
</template>
