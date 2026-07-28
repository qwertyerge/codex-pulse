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
  let lifecycleGeneration = 0;
  let operationToken: symbol | undefined;

  async function closeUpdate(update: UpdateCandidate | undefined) {
    if (!update) return;
    try {
      await update.close();
    } catch {
      // Closing a stale resource must not replace the current state.
    }
  }

  async function closeCandidate() {
    const stale = candidate;
    candidate = undefined;
    await closeUpdate(stale);
  }

  function ownsOperation(generation: number, token: symbol) {
    return (
      started &&
      lifecycleGeneration === generation &&
      operationToken === token
    );
  }

  function blocksCheck() {
    return (
      operationToken !== undefined ||
      state.value.phase === "downloading" ||
      state.value.phase === "ready" ||
      state.value.phase === "installing"
    );
  }

  async function checkForUpdate() {
    if (!started || !runtime.enabled || blocksCheck()) return;
    const generation = lifecycleGeneration;
    const token = Symbol();
    operationToken = token;
    let stage: UpdaterFailureStage = "check";
    state.value = { phase: "checking" };

    try {
      const update = await runtime.check();
      if (!ownsOperation(generation, token)) {
        await closeUpdate(update ?? undefined);
        return;
      }
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
        if (!ownsOperation(generation, token)) return;

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

      if (!ownsOperation(generation, token)) return;
      state.value = { phase: "ready", version: update.version };
    } catch {
      if (!ownsOperation(generation, token)) return;
      await closeCandidate();
      if (!ownsOperation(generation, token)) return;
      state.value = { phase: "failed", stage };
    } finally {
      if (operationToken === token) operationToken = undefined;
    }
  }

  async function activate(copy: UpdateConfirmationCopy) {
    if (state.value.phase === "failed") {
      await checkForUpdate();
      return;
    }
    if (
      !started ||
      state.value.phase !== "ready" ||
      !candidate ||
      operationToken !== undefined
    ) {
      return;
    }

    const update = candidate;
    const version = state.value.version;
    const generation = lifecycleGeneration;
    const token = Symbol();
    operationToken = token;
    let stage: UpdaterFailureStage = "confirm";

    try {
      const accepted = await runtime.confirm(copy.message, {
        title: copy.title,
        kind: "info"
      });
      if (!ownsOperation(generation, token) || !accepted) return;

      stage = "install";
      state.value = { phase: "installing", version };
      await update.install();
      if (!ownsOperation(generation, token)) return;

      stage = "relaunch";
      await closeCandidate();
      if (!ownsOperation(generation, token)) return;
      await runtime.relaunch();
    } catch {
      if (!ownsOperation(generation, token)) return;
      await closeCandidate();
      if (!ownsOperation(generation, token)) return;
      state.value = { phase: "failed", stage };
    } finally {
      if (operationToken === token) operationToken = undefined;
    }
  }

  function start() {
    if (started || !runtime.enabled) return;
    started = true;
    lifecycleGeneration += 1;
    void checkForUpdate();
    timer = setInterval(() => {
      void checkForUpdate();
    }, UPDATE_CHECK_INTERVAL_MS);
  }

  function stop() {
    started = false;
    lifecycleGeneration += 1;
    if (timer) clearInterval(timer);
    timer = undefined;
    operationToken = undefined;
    state.value = { phase: "idle" };
    void closeCandidate();
  }

  return {
    state: readonly(state),
    start,
    stop,
    activate
  };
}
