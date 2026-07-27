<script setup lang="ts">
import { GitBranch } from "@lucide/vue";
import { computed, nextTick, onBeforeUnmount, onMounted, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { projectName } from "../lib/projectName";
import type { SessionGitContext } from "../types";

const props = defineProps<{ cwd: string; git?: SessionGitContext }>();
defineEmits<{ "open-project": [path: string] }>();
const { t } = useI18n();

const anchor = ref<HTMLElement>();
const popup = ref<HTMLElement>();
const popupOpen = ref(false);
const hovered = ref(false);
const focused = ref(false);
const popupId = `project-${useId()}`;
const placement = ref<"above" | "below">("below");
const popupStyle = ref<Record<string, string>>({});
const displayedProjectName = computed(
  () => props.git?.projectName || projectName(props.cwd)
);
const displayedBranch = computed(
  () => props.git?.branch || t("session.noBranch")
);

function positionPopup() {
  if (!popupOpen.value || !anchor.value) return;
  const rect = anchor.value.getBoundingClientRect();
  const padding = 12;
  const gap = 8;
  const width = Math.max(0, Math.min(280, window.innerWidth - padding * 2));
  const height = popup.value?.offsetHeight || 112;
  const fitsBelow = rect.bottom + gap + height <= window.innerHeight - padding;
  placement.value = fitsBelow ? "below" : "above";
  const top = fitsBelow
    ? rect.bottom + gap
    : Math.max(padding, rect.top - gap - height);
  const left = Math.min(
    Math.max(padding, rect.left),
    Math.max(padding, window.innerWidth - padding - width)
  );
  popupStyle.value = {
    top: `${Math.round(top)}px`,
    left: `${Math.round(left)}px`,
    width: `${Math.round(width)}px`
  };
}

async function openPopup() {
  if (!props.git) return;
  popupOpen.value = true;
  await nextTick();
  positionPopup();
}

function hidePopupIfInactive() {
  if (!hovered.value && !focused.value) popupOpen.value = false;
}

async function handleMouseEnter() {
  hovered.value = true;
  await openPopup();
}

function handleMouseLeave() {
  hovered.value = false;
  hidePopupIfInactive();
}

async function handleFocus() {
  focused.value = true;
  await openPopup();
}

function handleBlur() {
  focused.value = false;
  hidePopupIfInactive();
}

onMounted(() => {
  window.addEventListener("resize", positionPopup);
  window.addEventListener("scroll", positionPopup, true);
});

onBeforeUnmount(() => {
  window.removeEventListener("resize", positionPopup);
  window.removeEventListener("scroll", positionPopup, true);
});
</script>

<template>
  <span class="session-card__project">
    <a
      ref="anchor"
      class="session-card__path"
      href="#"
      :aria-describedby="git && popupOpen ? popupId : undefined"
      @click.prevent="$emit('open-project', cwd)"
      @mouseenter="handleMouseEnter"
      @mouseleave="handleMouseLeave"
      @focus="handleFocus"
      @blur="handleBlur"
    >{{ displayedProjectName }}</a>
    <span v-if="git" class="session-card__branch">
      <GitBranch aria-hidden="true" />
      <span>{{ displayedBranch }}</span>
    </span>
  </span>
  <Teleport to="body">
    <aside
      v-if="git && popupOpen"
      :id="popupId"
      ref="popup"
      class="project-hover-card"
      role="tooltip"
      :data-placement="placement"
      :style="popupStyle"
    >
      <strong>{{ displayedProjectName }}</strong>
      <dl>
        <div>
          <dt>{{ t("session.defaultBranch") }}</dt>
          <dd>{{ git.defaultBranch || t("session.notConfigured") }}</dd>
        </div>
        <div>
          <dt>{{ t("session.remoteRepository") }}</dt>
          <dd>{{ git.remoteUrl || t("session.notConfigured") }}</dd>
        </div>
      </dl>
    </aside>
  </Teleport>
</template>
