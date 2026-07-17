export interface SessionSnapshot {
  threadId: string;
  title: string;
  cwd: string;
  sessionCreatedAtMs: number;
  currentRunStartedAtMs: number;
  recentEvent?: RecentEvent;
  lastUserMessage?: UserMessage;
}
export interface UserMessage { content: string; occurredAtMs: number; }

export interface RecentEvent {
  summary: string;
  detail?: string;
  occurredAtMs: number;
}

export interface MonitoringView {
  enabled: boolean;
  needsRepair: boolean;
  staleCount: number;
  degradedReason?: string;
}

export interface WeeklyQuota {
  usedPercent: number;
  remainingPercent: number;
  resetsAtMs: number;
}

export type ThemeMode = "system" | "light" | "dark";
export type LocaleMode = "system" | "zh-CN" | "en" | "fr" | "de";
export type InitializationPhase = "idle" | "starting" | "discoveringCandidates" | "readingQuota" | "reconcilingSessions" | "complete" | "failed";
export interface InitializationEvent {
  runId: number;
  sequence: number;
  occurredAtMs: number;
  phase: InitializationPhase;
  summary: string;
}
export interface InitializationSnapshot {
  runId: number;
  phase: InitializationPhase;
  events: InitializationEvent[];
}

export interface AppSnapshot {
  sessions: SessionSnapshot[];
  weeklyQuota?: WeeklyQuota;
  isLoading: boolean;
  initialization: InitializationSnapshot;
  monitoring: MonitoringView;
  alwaysOnTop: boolean;
  launchAtLogin: boolean;
  locale: LocaleMode;
  theme: ThemeMode;
}
