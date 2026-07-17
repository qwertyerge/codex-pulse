<script setup lang="ts">
import DOMPurify from "dompurify";
import { marked } from "marked";
import { computed } from "vue";
import { invoke } from "@tauri-apps/api/core";

const props = defineProps<{ source: string }>();
const markdownRenderer = new marked.Renderer();
markdownRenderer.html = () => "";
markdownRenderer.image = ({ href, title, text }) => {
  const label = title || text || "Image";
  return `<a class="markdown-image-placeholder" href="${escapeHtml(href)}" title="${escapeHtml(label)}"><span class="markdown-image-placeholder__icon" aria-hidden="true">▧</span><span>${escapeHtml(label)}</span></a>`;
};

function escapeHtml(value: string) {
  return value.replace(/[&<>"']/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[character] ?? character);
}

function handoffExternalLink(event: MouseEvent) {
  if (!(event.target instanceof Element)) return;
  const link = event.target.closest<HTMLAnchorElement>("a[href]");
  const url = link?.getAttribute("href");
  if (!url) return;
  event.preventDefault();
  void invoke("open_external_url", { url });
}

const html = computed(() => {
  const markdownOnly = marked.parse(props.source, {
    async: false,
    renderer: markdownRenderer
  }) as string;
  return DOMPurify.sanitize(markdownOnly, {
    ALLOWED_TAGS: ["a", "blockquote", "br", "code", "del", "em", "h1", "h2", "h3", "h4", "h5", "h6", "hr", "li", "ol", "p", "pre", "span", "strong", "ul"],
    ALLOWED_ATTR: ["aria-hidden", "class", "href", "title"]
  });
});
</script>

<template>
  <div class="markdown-content" v-html="html" @click="handoffExternalLink" />
</template>
