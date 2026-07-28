import { createI18n } from "vue-i18n";
import type { LocaleMode } from "./types";

export type ResolvedLocale = Exclude<LocaleMode, "system">;

export const messages = {
  en: {
    topBar: { active: "{count} active", appearance: "Appearance", light: "Use light appearance", dark: "Use dark appearance", system: "Follow system appearance", language: "Choose language", pin: "Pin window to top", unpin: "Unpin window" },
    updater: { downloading: "Update {percent}%", downloadingUnknown: "Updating", ready: "Update", installing: "Updating", failed: "Update failed", readyTitle: "Install version {version}", retryTitle: "Retry update", confirmTitle: "Install Codex Pulse update", confirmMessage: "Version {version} is ready. Install it and restart Codex Pulse?" },
    language: { system: "System", zhCn: "中文", en: "English", fr: "Français", de: "Deutsch" },
    empty: { loadingLabel: "Loading Codex sessions", emptyLabel: "No active Codex sessions", loadingTitle: "Loading active Codex sessions", emptyTitle: "No active Codex sessions", loadingDescription: "Reconciling recent Codex activity…", emptyDescription: "Waiting for a running Codex task." },
    monitoring: { repair: "Monitoring needs repair", degraded: "Monitoring degraded", disabled: "Live monitoring is not enabled yet", fallback: "Codex Pulse will continue using read-only JSONL reconciliation.", enableHint: "Session files remain read-only until you enable lifecycle hooks.", enable: "Enable hooks" },
    quota: { label: "Weekly quota", usedRemainingPrefix: "Used {used}% · Remaining", resets: "Resets in {countdown}", stale: "No active sessions; updates resume automatically when a task starts", unavailable: "Weekly quota · unavailable", aria: "Codex weekly quota", progressAria: "Codex weekly quota used" },
    session: { open: "Open Codex task: {title}", currentRun: "Current run", sessionAge: "Session age", lastPrompt: "Last prompt", recent: "Recent", ago: "ago", expandRecent: "Expand recent event", collapseRecent: "Collapse recent event", paused: "Recent age paused", pausedTitle: "Recent age paused while expanded", noBranch: "No branch", defaultBranch: "Default branch", remoteRepository: "Remote repository", notConfigured: "Not configured" },
    initialization: { feedAria: "Codex Pulse initialization progress", backgroundAria: "Codex Pulse background refresh", starting: "Starting refresh", discoveringCandidates: "Discovering recent active-session candidates", readingQuota: "Reading weekly quota", reconcilingSessions: "Reconciling active Codex sessions", complete: "Active session reconciliation complete", failed: "Refresh failed" },
    markdown: { image: "Image" },
    list: { end: "END" }
  },
  "zh-CN": {
    topBar: { active: "{count} 个活跃", appearance: "外观", light: "使用浅色外观", dark: "使用深色外观", system: "跟随系统外观", language: "选择语言", pin: "窗口置顶", unpin: "取消窗口置顶" },
    updater: { downloading: "更新 {percent}%", downloadingUnknown: "更新中", ready: "更新", installing: "更新中", failed: "更新失败", readyTitle: "安装版本 {version}", retryTitle: "重试更新", confirmTitle: "安装 Codex Pulse 更新", confirmMessage: "版本 {version} 已准备好。现在安装并重启 Codex Pulse？" },
    language: { system: "跟随系统", zhCn: "中文", en: "English", fr: "Français", de: "Deutsch" },
    empty: { loadingLabel: "正在加载 Codex 会话", emptyLabel: "没有活跃的 Codex 会话", loadingTitle: "正在加载活跃的 Codex 会话", emptyTitle: "没有活跃的 Codex 会话", loadingDescription: "正在核对最近的 Codex 活动…", emptyDescription: "等待运行中的 Codex 任务。" },
    monitoring: { repair: "监控需要修复", degraded: "监控已降级", disabled: "实时监控尚未启用", fallback: "Codex Pulse 会继续使用只读 JSONL 核对。", enableHint: "启用生命周期钩子前，会话文件保持只读。", enable: "启用钩子" },
    quota: { label: "周额度", usedRemainingPrefix: "已用 {used}% · 剩余", resets: "{countdown} 后重置", stale: "暂无活跃会话；新任务开始后将自动恢复更新", unavailable: "周额度 · 暂不可用", aria: "Codex 周额度", progressAria: "Codex 周额度已用" },
    session: { open: "打开 Codex 任务：{title}", currentRun: "当前运行", sessionAge: "会话时长", lastPrompt: "最近提示", recent: "最近事件", ago: "前", expandRecent: "展开最近事件", collapseRecent: "折叠最近事件", paused: "最近事件计时已暂停", pausedTitle: "展开时最近事件计时已暂停", noBranch: "无分支", defaultBranch: "默认分支", remoteRepository: "远程仓库", notConfigured: "未配置" },
    initialization: { feedAria: "Codex Pulse 初始化进度", backgroundAria: "Codex Pulse 后台刷新", starting: "正在开始刷新", discoveringCandidates: "正在发现近期活跃会话", readingQuota: "正在读取周额度", reconcilingSessions: "正在核对活跃 Codex 会话", complete: "活跃会话核对完成", failed: "刷新失败" },
    markdown: { image: "图片" },
    list: { end: "结束" }
  },
  fr: {
    topBar: { active: "{count} actives", appearance: "Apparence", light: "Utiliser le thème clair", dark: "Utiliser le thème sombre", system: "Suivre le thème du système", language: "Choisir la langue", pin: "Épingler la fenêtre", unpin: "Désépingler la fenêtre" },
    updater: { downloading: "MàJ {percent} %", downloadingUnknown: "MàJ…", ready: "MàJ", installing: "MàJ…", failed: "Échec MàJ", readyTitle: "Installer la version {version}", retryTitle: "Réessayer la mise à jour", confirmTitle: "Installer la mise à jour de Codex Pulse", confirmMessage: "La version {version} est prête. L’installer et redémarrer Codex Pulse ?" },
    language: { system: "Système", zhCn: "中文", en: "English", fr: "Français", de: "Deutsch" },
    empty: { loadingLabel: "Chargement des sessions Codex", emptyLabel: "Aucune session Codex active", loadingTitle: "Chargement des sessions Codex actives", emptyTitle: "Aucune session Codex active", loadingDescription: "Rapprochement de l’activité Codex récente…", emptyDescription: "En attente d’une tâche Codex en cours." },
    monitoring: { repair: "La surveillance doit être réparée", degraded: "Surveillance dégradée", disabled: "La surveillance en direct n’est pas encore activée", fallback: "Codex Pulse continue le rapprochement JSONL en lecture seule.", enableHint: "Les fichiers de session restent en lecture seule jusqu’à l’activation des hooks de cycle de vie.", enable: "Activer les hooks" },
    quota: { label: "Quota hebdomadaire", usedRemainingPrefix: "Utilisé {used}% · Restant", resets: "Réinitialisation dans {countdown}", stale: "Aucune session active ; les mises à jour reprendront au démarrage d’une tâche", unavailable: "Quota hebdomadaire · indisponible", aria: "Quota hebdomadaire Codex", progressAria: "Quota hebdomadaire Codex utilisé" },
    session: { open: "Ouvrir la tâche Codex : {title}", currentRun: "Exécution en cours", sessionAge: "Âge de la session", lastPrompt: "Dernier prompt", recent: "Récent", ago: "plus tôt", expandRecent: "Développer l’événement récent", collapseRecent: "Réduire l’événement récent", paused: "Âge récent en pause", pausedTitle: "Âge récent en pause pendant le développement", noBranch: "Aucune branche", defaultBranch: "Branche par défaut", remoteRepository: "Dépôt distant", notConfigured: "Non configuré" },
    initialization: { feedAria: "Progression de l’initialisation de Codex Pulse", backgroundAria: "Actualisation en arrière-plan de Codex Pulse", starting: "Démarrage de l’actualisation", discoveringCandidates: "Recherche des sessions actives récentes", readingQuota: "Lecture du quota hebdomadaire", reconcilingSessions: "Rapprochement des sessions Codex actives", complete: "Rapprochement des sessions actives terminé", failed: "Échec de l’actualisation" },
    markdown: { image: "Image" },
    list: { end: "FIN" }
  },
  de: {
    topBar: { active: "{count} aktiv", appearance: "Darstellung", light: "Helles Erscheinungsbild verwenden", dark: "Dunkles Erscheinungsbild verwenden", system: "Systemdarstellung verwenden", language: "Sprache auswählen", pin: "Fenster anheften", unpin: "Fenster lösen" },
    updater: { downloading: "Update {percent} %", downloadingUnknown: "Update läuft", ready: "Update", installing: "Update läuft", failed: "Updatefehler", readyTitle: "Version {version} installieren", retryTitle: "Update erneut versuchen", confirmTitle: "Codex-Pulse-Update installieren", confirmMessage: "Version {version} ist bereit. Jetzt installieren und Codex Pulse neu starten?" },
    language: { system: "System", zhCn: "中文", en: "English", fr: "Français", de: "Deutsch" },
    empty: { loadingLabel: "Codex-Sitzungen werden geladen", emptyLabel: "Keine aktiven Codex-Sitzungen", loadingTitle: "Aktive Codex-Sitzungen werden geladen", emptyTitle: "Keine aktiven Codex-Sitzungen", loadingDescription: "Letzte Codex-Aktivitäten werden abgeglichen…", emptyDescription: "Warte auf eine laufende Codex-Aufgabe." },
    monitoring: { repair: "Überwachung muss repariert werden", degraded: "Überwachung eingeschränkt", disabled: "Live-Überwachung ist noch nicht aktiviert", fallback: "Codex Pulse verwendet weiterhin den schreibgeschützten JSONL-Abgleich.", enableHint: "Sitzungsdateien bleiben schreibgeschützt, bis Lifecycle-Hooks aktiviert sind.", enable: "Hooks aktivieren" },
    quota: { label: "Wochenkontingent", usedRemainingPrefix: "Verwendet {used}% · Verbleibend", resets: "Zurücksetzen in {countdown}", stale: "Keine aktiven Sitzungen; Updates werden beim Start einer Aufgabe fortgesetzt", unavailable: "Wochenkontingent · nicht verfügbar", aria: "Codex-Wochenkontingent", progressAria: "Verwendetes Codex-Wochenkontingent" },
    session: { open: "Codex-Aufgabe öffnen: {title}", currentRun: "Aktueller Lauf", sessionAge: "Sitzungsalter", lastPrompt: "Letzte Eingabe", recent: "Kürzlich", ago: "her", expandRecent: "Letztes Ereignis ausklappen", collapseRecent: "Letztes Ereignis einklappen", paused: "Aktuelles Alter pausiert", pausedTitle: "Aktuelles Alter ist beim Ausklappen pausiert", noBranch: "Kein Branch", defaultBranch: "Standardbranch", remoteRepository: "Remote-Repository", notConfigured: "Nicht konfiguriert" },
    initialization: { feedAria: "Codex-Pulse-Initialisierungsfortschritt", backgroundAria: "Codex-Pulse-Hintergrundaktualisierung", starting: "Aktualisierung wird gestartet", discoveringCandidates: "Kürzlich aktive Sitzungen werden gesucht", readingQuota: "Wochenkontingent wird gelesen", reconcilingSessions: "Aktive Codex-Sitzungen werden abgeglichen", complete: "Abgleich aktiver Sitzungen abgeschlossen", failed: "Aktualisierung fehlgeschlagen" },
    markdown: { image: "Bild" },
    list: { end: "ENDE" }
  }
} as const;

export function resolveLocale(preference: LocaleMode, browserLanguage = navigator.language): ResolvedLocale {
  if (preference !== "system") return preference;
  const language = browserLanguage.toLowerCase();
  if (language.startsWith("zh")) return "zh-CN";
  if (language.startsWith("fr")) return "fr";
  if (language.startsWith("de")) return "de";
  return "en";
}

export const i18n = createI18n({
  legacy: false,
  locale: "en",
  fallbackLocale: "en",
  messages
});
