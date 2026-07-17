<script setup lang="ts">
defineProps<{ enabled: boolean; needsRepair: boolean; degradedReason?: string }>();
defineEmits<{ enable: [] }>();
</script>

<template>
  <aside v-if="degradedReason || needsRepair || !enabled" class="monitoring-banner" aria-live="polite">
    <strong v-if="needsRepair">Monitoring needs repair</strong>
    <strong v-else-if="degradedReason">Monitoring degraded</strong>
    <strong v-else>Live monitoring is not enabled yet</strong>
    <span v-if="degradedReason">{{ degradedReason }}</span>
    <span v-else-if="needsRepair">Codex Pulse will continue using read-only JSONL reconciliation.</span>
    <span v-else>Session files remain read-only until you enable lifecycle hooks.</span>
    <button v-if="!enabled && !needsRepair" type="button" @click="$emit('enable')">Enable hooks</button>
  </aside>
</template>
