import type { Composer } from "vue-i18n";
import type { InitializationEvent } from "../types";

const phaseKey = {
  starting: "initialization.starting",
  discoveringCandidates: "initialization.discoveringCandidates",
  readingQuota: "initialization.readingQuota",
  reconcilingSessions: "initialization.reconcilingSessions",
  complete: "initialization.complete"
} as const;

export function initializationLabel(t: Composer["t"], event: InitializationEvent) {
  const key = phaseKey[event.phase as keyof typeof phaseKey];
  return key ? t(key) : event.summary;
}
