<script setup lang="ts">
import { useI18n } from "vue-i18n";
import InitializationFeed from "./InitializationFeed.vue";
import type { InitializationSnapshot } from "../types";

const props = defineProps<{ loading: boolean; initialization: InitializationSnapshot }>();
const { t } = useI18n();
</script>

<template>
  <section class="empty-state" :aria-label="props.loading ? t('empty.loadingLabel') : t('empty.emptyLabel')">
    <svg viewBox="0 0 24 24" aria-hidden="true">
      <path d="M3 13h5l2-5 4 10 2-5h5" />
    </svg>
    <strong>{{ props.loading ? t("empty.loadingTitle") : t("empty.emptyTitle") }}</strong>
    <span>{{ props.loading ? t("empty.loadingDescription") : t("empty.emptyDescription") }}</span>
    <InitializationFeed v-if="props.loading && props.initialization.runId === 1 && props.initialization.events.length" :initialization="props.initialization" />
  </section>
</template>
