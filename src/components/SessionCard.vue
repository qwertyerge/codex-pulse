<script setup lang="ts">
import { computed, ref } from "vue";
import { formatDuration, formatRecentAge } from "../lib/duration";
import type { RecentEvent, SessionSnapshot, UserMessage } from "../types";

const props = defineProps<{ session: SessionSnapshot; nowMs: number }>();
defineEmits<{ open: [threadId: string] }>();

const expanded = ref(false);
const frozenEvent = ref<RecentEvent>();
const promptExpanded = ref(false);
const frozenPrompt = ref<UserMessage>();
const displayedPrompt = computed(() => promptExpanded.value ? frozenPrompt.value : props.session.lastUserMessage);
const displayedEvent = computed(() => expanded.value ? frozenEvent.value : props.session.recentEvent);
const currentRun = computed(() => formatDuration(props.nowMs - props.session.currentRunStartedAtMs));
const sessionAge = computed(() => formatDuration(props.nowMs - props.session.sessionCreatedAtMs));
const recentEventAge = computed(() => displayedEvent.value && formatRecentAge(props.nowMs - displayedEvent.value.occurredAtMs));

function toggleRecentEvent() {
  if (expanded.value) {
    expanded.value = false;
    frozenEvent.value = undefined;
    return;
  }
  frozenEvent.value = props.session.recentEvent;
  expanded.value = true;
}
function togglePrompt() { if (promptExpanded.value) { promptExpanded.value = false; frozenPrompt.value = undefined; } else { frozenPrompt.value = props.session.lastUserMessage; promptExpanded.value = true; } }
</script>

<template>
  <article class="session-card">
    <button
      class="session-card__main"
      type="button"
      :aria-label="`Open Codex task: ${session.title}`"
      @click="$emit('open', session.threadId)"
    >
      <span class="session-card__heading">
        <span class="session-card__active-dot" aria-hidden="true" />
        <span class="session-card__title" :title="session.title">{{ session.title }}</span>
        <svg class="session-card__open-icon" viewBox="0 0 20 20" aria-hidden="true">
          <path d="M11 3h6v6M17 3l-8 8M15 11v5a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1h5" />
        </svg>
      </span>
      <span class="session-card__path" :title="session.cwd">{{ session.cwd }}</span>
      <span class="session-card__timers">
        <span class="session-card__timer"><small>Current run</small><strong>{{ currentRun }}</strong></span>
        <span class="session-card__timer"><small>Session age</small><strong>{{ sessionAge }}</strong></span>
      </span>
    </button>
    <div v-if="displayedPrompt" class="session-card__meta">
      <button class="session-card__meta-row" type="button" :aria-expanded="promptExpanded" @click="togglePrompt"><span class="session-card__recent"><small>Last prompt</small><span class="session-card__recent-summary">{{ promptExpanded ? '' : displayedPrompt.content }}</span></span><span class="session-card__meta-toggle" aria-hidden="true">{{ promptExpanded ? '⌃' : '⌄' }}</span></button>
      <p v-if="promptExpanded" class="session-card__recent-detail">{{ displayedPrompt.content }}</p>
    </div>
    <div v-if="displayedEvent" class="session-card__meta">
      <button class="session-card__meta-row session-card__recent-toggle" type="button" :aria-expanded="expanded" :title="expanded ? 'Collapse recent event' : 'Expand recent event'" @click="toggleRecentEvent"><span class="session-card__recent"><small class="session-card__recent-time"><span>Recent · {{ recentEventAge }}</span><i aria-hidden="true">Recent · 99d+</i></small><span class="session-card__recent-summary">{{ expanded ? '' : displayedEvent.summary }}</span></span><span class="session-card__meta-toggle" aria-hidden="true">{{ expanded ? '⌃' : '⌄' }}</span></button>
      <p v-if="expanded" class="session-card__recent-detail">{{ displayedEvent.detail || displayedEvent.summary }}</p>
    </div>
  </article>
</template>
