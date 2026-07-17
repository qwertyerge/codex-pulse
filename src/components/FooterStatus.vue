<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { formatQuotaReset } from "../lib/duration";
import type { WeeklyQuota } from "../types";

const props = defineProps<{ quota?: WeeklyQuota; nowMs: number; activeSessionCount: number }>();
const { t } = useI18n();

const isAvailable = computed(() => Boolean(props.quota && props.quota.resetsAtMs > props.nowMs));
const isStale = computed(() => isAvailable.value && props.activeSessionCount === 0);
const usedPercent = computed(() => Math.min(100, Math.max(0, props.quota?.usedPercent ?? 0)));
const resetCountdown = computed(() => isAvailable.value && props.quota && formatQuotaReset(props.quota.resetsAtMs - props.nowMs));
</script>

<template>
  <footer
    class="quota-footer"
    :class="{ 'quota-footer--unavailable': !isAvailable, 'quota-footer--stale': isStale }"
    :aria-label="t('quota.aria')"
  >
    <svg class="quota-footer__icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
      <path d="M6 3h12M6 21h12M7 3c0 4 2 5 5 9-3 4-5 5-5 9M17 3c0 4-2 5-5 9 3 4 5 5 5 9M9.2 16h5.6" />
    </svg>
    <template v-if="isAvailable && quota">
      <div v-if="isStale" class="quota-footer__content quota-footer__stale-content">
        <div class="quota-footer__line">
          <strong>{{ t("quota.label") }}</strong>
          <span>{{ t("quota.usedRemainingPrefix", { used: usedPercent }) }} <b class="quota-footer__remaining-value">{{ quota.remainingPercent }}%</b></span>
          <time>{{ t("quota.resets", { countdown: resetCountdown }) }}</time>
        </div>
        <p>{{ t("quota.stale") }}</p>
      </div>
      <div v-else class="quota-footer__content">
        <div class="quota-footer__line">
          <strong>{{ t("quota.label") }}</strong>
          <span>{{ t("quota.usedRemainingPrefix", { used: usedPercent }) }} <b class="quota-footer__remaining-value">{{ quota.remainingPercent }}%</b></span>
          <time>{{ t("quota.resets", { countdown: resetCountdown }) }}</time>
        </div>
        <div
          class="quota-footer__progress"
          role="progressbar"
          :aria-label="t('quota.progressAria')"
          aria-valuemin="0"
          aria-valuemax="100"
          :aria-valuenow="usedPercent"
        >
          <span class="quota-footer__progress-fill" :style="{ width: `${usedPercent}%` }" />
        </div>
      </div>
    </template>
    <span v-else class="quota-footer__unavailable">{{ t("quota.unavailable") }}</span>
  </footer>
</template>
