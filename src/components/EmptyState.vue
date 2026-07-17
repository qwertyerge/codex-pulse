<script setup lang="ts">
defineProps<{ loading: boolean }>();
</script>

<template>
  <section class="empty-state" :aria-label="loading ? 'Loading Codex sessions' : 'No active Codex sessions'">
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M3 13h5l2-5 4 10 2-5h5" />
    </svg>
    <strong>{{ loading ? "Loading active Codex sessions" : "No active Codex sessions" }}</strong>
    <span>{{ loading ? "Reconciling recent Codex activity…" : "Waiting for a running Codex task." }}</span>
  </section>
</template>
