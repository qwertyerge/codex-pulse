<script setup lang="ts">
import { useI18n } from "vue-i18n";

defineProps<{ enabled: boolean; needsRepair: boolean; degradedReason?: string }>();
defineEmits<{ enable: [] }>();
const { t } = useI18n();
</script>

<template>
  <aside v-if="degradedReason || needsRepair || !enabled" class="monitoring-banner" aria-live="polite">
    <strong v-if="needsRepair">{{ t("monitoring.repair") }}</strong>
    <strong v-else-if="degradedReason">{{ t("monitoring.degraded") }}</strong>
    <strong v-else>{{ t("monitoring.disabled") }}</strong>
    <span v-if="degradedReason">{{ degradedReason }}</span>
    <span v-else-if="needsRepair">{{ t("monitoring.fallback") }}</span>
    <span v-else>{{ t("monitoring.enableHint") }}</span>
    <button v-if="!enabled && !needsRepair" type="button" @click="$emit('enable')">{{ t("monitoring.enable") }}</button>
  </aside>
</template>
