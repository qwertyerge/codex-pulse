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

export interface AppSnapshot {
  sessions: SessionSnapshot[];
  isLoading: boolean;
  monitoring: MonitoringView;
  alwaysOnTop: boolean;
  launchAtLogin: boolean;
  locale: "system" | "en" | "zh-CN";
}
