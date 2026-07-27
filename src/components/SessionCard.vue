<script setup lang="ts">
import { ChevronDown, ChevronUp, ExternalLink, Pause } from "@lucide/vue";
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { formatDuration, formatRecentAgeValue } from "../lib/duration";
import { measureRecentAgeWidth } from "../lib/recentAgeWidth";
import type { RecentEvent, SessionSnapshot, UserMessage } from "../types";
import MarkdownContent from "./MarkdownContent.vue";
import ProjectIdentity from "./ProjectIdentity.vue";

const props = defineProps<{ session: SessionSnapshot; nowMs: number }>();
defineEmits<{
  open: [threadId: string];
  "open-project": [path: string];
}>();
const { t } = useI18n();

const expanded = ref(false);
const frozenEvent = ref<RecentEvent>();
const frozenRecentAge = ref<string>();
const promptExpanded = ref(false);
const frozenPrompt = ref<UserMessage>();
const recentAgeLabel = ref<HTMLElement>();
const recentAgeMeasure = ref<HTMLElement>();
const recentAgeWidth = ref(0);
const displayedPrompt = computed(() => promptExpanded.value ? frozenPrompt.value : props.session.lastUserMessage);
const displayedEvent = computed(() => expanded.value ? frozenEvent.value : props.session.recentEvent);
const currentRun = computed(() => formatDuration(props.nowMs - props.session.currentRunStartedAtMs));
const sessionAge = computed(() => formatDuration(props.nowMs - props.session.sessionCreatedAtMs));
const recentEventAge = computed(() => frozenRecentAge.value || (displayedEvent.value && formatRecentAgeValue(props.nowMs - displayedEvent.value.occurredAtMs)));

onMounted(() => {
  if (recentAgeMeasure.value) recentAgeWidth.value = measureRecentAgeWidth(recentAgeMeasure.value);
});

function toggleRecentEvent() {
  if (expanded.value) {
    expanded.value = false;
    frozenEvent.value = undefined;
    frozenRecentAge.value = undefined;
    return;
  }
  frozenEvent.value = props.session.recentEvent;
  frozenRecentAge.value = props.session.recentEvent && formatRecentAgeValue(props.nowMs - props.session.recentEvent.occurredAtMs);
  expanded.value = true;
}

function togglePrompt() {
  if (promptExpanded.value) {
    promptExpanded.value = false;
    frozenPrompt.value = undefined;
    return;
  }
  frozenPrompt.value = props.session.lastUserMessage;
  promptExpanded.value = true;
}
</script>

<template>
  <article class="session-card">
    <div class="session-card__main">
      <span class="session-card__heading">
        <span class="session-card__active-dot" aria-hidden="true" />
        <span class="session-card__title" :title="session.title">{{ session.title }}</span>
        <button
          class="session-card__open"
          type="button"
          :aria-label="t('session.open', { title: session.title })"
          @click="$emit('open', session.threadId)"
        >
          <ExternalLink class="session-card__open-icon" aria-hidden="true" />
        </button>
      </span>
      <ProjectIdentity
        :cwd="session.cwd"
        :git="session.git"
        @open-project="$emit('open-project', $event)"
      />
      <span class="session-card__timers">
        <span class="session-card__timer"><small>{{ t("session.currentRun") }}</small><strong>{{ currentRun }}</strong></span>
        <span class="session-card__timer"><small>{{ t("session.sessionAge") }}</small><strong>{{ sessionAge }}</strong></span>
      </span>
    </div>
    <div v-if="displayedPrompt" class="session-card__meta">
      <button class="session-card__meta-row" type="button" :aria-expanded="promptExpanded" @click="togglePrompt">
        <span class="session-card__recent">
          <small>{{ t("session.lastPrompt") }}</small>
          <span v-if="!promptExpanded" class="session-card__recent-summary">{{ displayedPrompt.content }}</span>
        </span>
        <ChevronUp v-if="promptExpanded" class="session-card__meta-toggle" aria-hidden="true" />
        <ChevronDown v-else class="session-card__meta-toggle" aria-hidden="true" />
      </button>
      <MarkdownContent v-if="promptExpanded" class="session-card__recent-detail" :source="displayedPrompt.content" />
    </div>
    <div v-if="displayedEvent" class="session-card__meta">
      <button class="session-card__meta-row session-card__recent-toggle" type="button" :aria-expanded="expanded" :title="expanded ? t('session.collapseRecent') : t('session.expandRecent')" @click="toggleRecentEvent">
        <span class="session-card__recent">
          <small ref="recentAgeLabel" class="session-card__recent-time">
            <span>{{ t("session.recent") }} · </span>
            <span class="session-card__recent-age-value" :style="recentAgeWidth ? { width: `${recentAgeWidth}px` } : undefined">{{ recentEventAge }}</span>
            <span class="session-card__recent-age-suffix">&nbsp;{{ t("session.ago") }}</span>
            <span v-if="expanded" class="session-card__recent-paused" role="img" :aria-label="t('session.paused')" :title="t('session.pausedTitle')">
              <Pause aria-hidden="true" />
            </span>
            <span ref="recentAgeMeasure" class="session-card__recent-age-measure" aria-hidden="true">
              <span v-for="sample in ['1s', '10s', '1m', '10m', '1h', '10h', '1d', '10d', '99d+']" :key="sample" class="session-card__recent-age-value">{{ sample }}</span>
            </span>
          </small>
          <span v-if="!expanded" class="session-card__recent-summary">{{ displayedEvent.summary }}</span>
        </span>
        <ChevronUp v-if="expanded" class="session-card__meta-toggle" aria-hidden="true" />
        <ChevronDown v-else class="session-card__meta-toggle" aria-hidden="true" />
      </button>
      <MarkdownContent v-if="expanded" class="session-card__recent-detail" :source="displayedEvent.detail || displayedEvent.summary" />
    </div>
  </article>
</template>
