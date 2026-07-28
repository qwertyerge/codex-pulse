import { isTauri } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  check,
  type DownloadEvent
} from "@tauri-apps/plugin-updater";
import { readonly, ref } from "vue";

export const UPDATE_CHECK_INTERVAL_MS = 21_600_000;

export type UpdaterDownloadEvent = DownloadEvent;
export type UpdaterFailureStage =
  | "check"
  | "download"
  | "confirm"
  | "install"
  | "relaunch";

export type UpdaterState =
  | { phase: "idle" }
  | { phase: "checking" }
  | {
      phase: "downloading";
      version: string;
      downloaded: number;
      total?: number;
      percent?: number;
    }
  | { phase: "ready"; version: string }
  | { phase: "installing"; version: string }
  | { phase: "failed"; stage: UpdaterFailureStage };

export interface UpdateCandidate {
  version: string;
  download(
    onEvent?: (event: UpdaterDownloadEvent) => void
  ): Promise<void>;
  install(): Promise<void>;
  close(): Promise<void>;
}

export interface UpdaterRuntime {
  enabled: boolean;
  check(): Promise<UpdateCandidate | null>;
  confirm(
    message: string,
    options: { title: string; kind: "info" }
  ): Promise<boolean>;
  relaunch(): Promise<void>;
}

export interface UpdateConfirmationCopy {
  title: string;
  message: string;
}

const productionRuntime: UpdaterRuntime = {
  enabled: import.meta.env.PROD && isTauri(),
  check: async () => await check(),
  confirm,
  relaunch
};

export function useUpdater(runtime: UpdaterRuntime = productionRuntime) {
  const state = ref<UpdaterState>({ phase: "idle" });
  let candidate: UpdateCandidate | undefined;
  let timer: ReturnType<typeof setInterval> | undefined;
  let started = false;
  let inFlight = false;

  async function closeCandidate() {
    const stale = candidate;
    candidate = undefined;
    if (!stale) return;
    try {
      await stale.close();
    } catch {
      // Closing a stale resource must not replace the original state.
    }
  }

  function blocksCheck() {
    return (
      inFlight ||
      state.value.phase === "downloading" ||
      state.value.phase === "ready" ||
      state.value.phase === "installing"
    );
  }

  async function checkForUpdate() {
    if (!runtime.enabled || blocksCheck()) return;
    inFlight = true;
    let stage: UpdaterFailureStage = "check";
    state.value = { phase: "checking" };

    try {
      const update = await runtime.check();
      if (!update) {
        state.value = { phase: "idle" };
        return;
      }

      candidate = update;
      stage = "download";
      let downloaded = 0;
      let total: number | undefined;
      state.value = {
        phase: "downloading",
        version: update.version,
        downloaded
      };

      await update.download((event) => {
        if (event.event === "Started") {
          downloaded = 0;
          total =
            event.data.contentLength && event.data.contentLength > 0
              ? event.data.contentLength
              : undefined;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
        }

        if (event.event !== "Finished") {
          const percent = total
            ? Math.min(
                100,
                Math.max(0, Math.floor((downloaded / total) * 100))
              )
            : undefined;
          state.value = {
            phase: "downloading",
            version: update.version,
            downloaded,
            ...(total === undefined ? {} : { total }),
            ...(percent === undefined ? {} : { percent })
          };
        }
      });

      state.value = { phase: "ready", version: update.version };
    } catch {
      await closeCandidate();
      state.value = { phase: "failed", stage };
    } finally {
      inFlight = false;
    }
  }

  async function activate(copy: UpdateConfirmationCopy) {
    if (state.value.phase === "failed") {
      await checkForUpdate();
      return;
    }
    if (state.value.phase !== "ready" || !candidate || inFlight) return;

    const update = candidate;
    const version = state.value.version;
    inFlight = true;
    let stage: UpdaterFailureStage = "confirm";

    try {
      const accepted = await runtime.confirm(copy.message, {
        title: copy.title,
        kind: "info"
      });
      if (!accepted) return;

      stage = "install";
      state.value = { phase: "installing", version };
      await update.install();

      stage = "relaunch";
      await closeCandidate();
      await runtime.relaunch();
    } catch {
      await closeCandidate();
      state.value = { phase: "failed", stage };
    } finally {
      inFlight = false;
    }
  }

  function start() {
    if (started || !runtime.enabled) return;
    started = true;
    void checkForUpdate();
    timer = setInterval(() => {
      void checkForUpdate();
    }, UPDATE_CHECK_INTERVAL_MS);
  }

  function stop() {
    started = false;
    if (timer) clearInterval(timer);
    timer = undefined;
    void closeCandidate();
  }

  return {
    state: readonly(state),
    start,
    stop,
    activate
  };
}
