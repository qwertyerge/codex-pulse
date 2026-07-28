import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi
} from "vitest";

import {
  UPDATE_CHECK_INTERVAL_MS,
  useUpdater,
  type UpdateCandidate,
  type UpdaterDownloadEvent,
  type UpdaterRuntime
} from "../composables/useUpdater";

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, resolve, reject };
}

function makeCandidate(
  version = "0.4.0",
  downloadGate?: ReturnType<typeof deferred<void>>
) {
  let listener: ((event: UpdaterDownloadEvent) => void) | undefined;
  const candidate: UpdateCandidate = {
    version,
    download: vi.fn(async (onEvent) => {
      listener = onEvent;
      if (downloadGate) await downloadGate.promise;
    }),
    install: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined)
  };
  return {
    candidate,
    emit(event: UpdaterDownloadEvent) {
      if (!listener) throw new Error("download listener is not registered");
      listener(event);
    }
  };
}

function makeRuntime(
  overrides: Partial<UpdaterRuntime> = {}
): UpdaterRuntime {
  return {
    enabled: true,
    check: vi.fn().mockResolvedValue(null),
    confirm: vi.fn().mockResolvedValue(true),
    relaunch: vi.fn().mockResolvedValue(undefined),
    ...overrides
  };
}

const confirmation = {
  title: "Install Codex Pulse update",
  message: "Version 0.4.0 is ready. Install it and restart Codex Pulse?"
};

describe("useUpdater", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("checks immediately, skips overlapping ticks, repeats after six hours, and stops", async () => {
    const firstCheck = deferred<null>();
    const check = vi
      .fn<UpdaterRuntime["check"]>()
      .mockReturnValueOnce(firstCheck.promise)
      .mockResolvedValue(null);
    const updater = useUpdater(makeRuntime({ check }));

    updater.start();
    updater.start();
    await vi.waitFor(() => expect(check).toHaveBeenCalledTimes(1));

    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);
    expect(check).toHaveBeenCalledTimes(1);

    firstCheck.resolve(null);
    await vi.waitFor(() =>
      expect(updater.state.value).toEqual({ phase: "idle" })
    );
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);
    expect(check).toHaveBeenCalledTimes(2);

    updater.stop();
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);
    expect(check).toHaveBeenCalledTimes(2);
  });

  it("stays idle and closes a late candidate when stopped during a check", async () => {
    const checkGate = deferred<UpdateCandidate | null>();
    const update = makeCandidate();
    const check = vi
      .fn<UpdaterRuntime["check"]>()
      .mockReturnValue(checkGate.promise);
    const updater = useUpdater(makeRuntime({ check }));

    updater.start();
    await vi.waitFor(() => expect(check).toHaveBeenCalledTimes(1));
    updater.stop();

    expect(updater.state.value).toEqual({ phase: "idle" });
    checkGate.resolve(update.candidate);
    await vi.waitFor(() =>
      expect(update.candidate.close).toHaveBeenCalledTimes(1)
    );

    expect(update.candidate.download).not.toHaveBeenCalled();
    expect(updater.state.value).toEqual({ phase: "idle" });
  });

  it("ignores download progress and completion after stop", async () => {
    const downloadGate = deferred<void>();
    const update = makeCandidate("0.4.0", downloadGate);
    const updater = useUpdater(
      makeRuntime({ check: vi.fn().mockResolvedValue(update.candidate) })
    );

    updater.start();
    await vi.waitFor(() =>
      expect(updater.state.value.phase).toBe("downloading")
    );
    updater.stop();

    expect(updater.state.value).toEqual({ phase: "idle" });
    update.emit({ event: "Started", data: { contentLength: 200 } });
    update.emit({ event: "Progress", data: { chunkLength: 84 } });
    expect(updater.state.value).toEqual({ phase: "idle" });

    downloadGate.resolve();
    await downloadGate.promise;
    await Promise.resolve();
    expect(update.candidate.close).toHaveBeenCalledTimes(1);
    expect(updater.state.value).toEqual({ phase: "idle" });
  });

  it("does not install when confirmation resolves after stop", async () => {
    const confirmationGate = deferred<boolean>();
    const update = makeCandidate();
    const runtime = makeRuntime({
      check: vi.fn().mockResolvedValue(update.candidate),
      confirm: vi.fn().mockReturnValue(confirmationGate.promise)
    });
    const updater = useUpdater(runtime);

    updater.start();
    await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
    const activation = updater.activate(confirmation);
    await vi.waitFor(() => expect(runtime.confirm).toHaveBeenCalledTimes(1));
    updater.stop();

    expect(updater.state.value).toEqual({ phase: "idle" });
    confirmationGate.resolve(true);
    await activation;

    expect(update.candidate.install).not.toHaveBeenCalled();
    expect(runtime.relaunch).not.toHaveBeenCalled();
    expect(updater.state.value).toEqual({ phase: "idle" });
  });

  it("keeps idle and suppresses relaunch when stopped during install", async () => {
    const installGate = deferred<void>();
    const update = makeCandidate();
    vi.mocked(update.candidate.install).mockReturnValue(installGate.promise);
    const runtime = makeRuntime({
      check: vi.fn().mockResolvedValue(update.candidate)
    });
    const updater = useUpdater(runtime);

    updater.start();
    await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
    const activation = updater.activate(confirmation);
    await vi.waitFor(() =>
      expect(update.candidate.install).toHaveBeenCalledTimes(1)
    );
    updater.stop();

    expect(updater.state.value).toEqual({ phase: "idle" });
    installGate.resolve();
    await activation;

    expect(runtime.relaunch).not.toHaveBeenCalled();
    expect(updater.state.value).toEqual({ phase: "idle" });
  });

  it("starts a fresh lifecycle while an old check is still pending", async () => {
    const firstCheck = deferred<UpdateCandidate | null>();
    const secondCheck = deferred<UpdateCandidate | null>();
    const stale = makeCandidate("0.4.0");
    const check = vi
      .fn<UpdaterRuntime["check"]>()
      .mockReturnValueOnce(firstCheck.promise)
      .mockReturnValueOnce(secondCheck.promise)
      .mockResolvedValue(null);
    const updater = useUpdater(makeRuntime({ check }));

    updater.start();
    await vi.waitFor(() => expect(check).toHaveBeenCalledTimes(1));
    updater.stop();
    updater.start();
    await vi.waitFor(() => expect(check).toHaveBeenCalledTimes(2));

    firstCheck.resolve(stale.candidate);
    await vi.waitFor(() =>
      expect(stale.candidate.close).toHaveBeenCalledTimes(1)
    );
    expect(stale.candidate.download).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);
    expect(check).toHaveBeenCalledTimes(2);

    secondCheck.resolve(null);
    await vi.waitFor(() =>
      expect(updater.state.value).toEqual({ phase: "idle" })
    );
  });

  it("does nothing when the runtime gate is disabled", async () => {
    const check = vi.fn<UpdaterRuntime["check"]>().mockResolvedValue(null);
    const updater = useUpdater(makeRuntime({ enabled: false, check }));

    updater.start();
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS * 2);

    expect(check).not.toHaveBeenCalled();
    expect(updater.state.value).toEqual({ phase: "idle" });
  });

  it("checks once and returns to idle when the current version is latest", async () => {
    const check = vi.fn<UpdaterRuntime["check"]>().mockResolvedValue(null);
    const updater = useUpdater(makeRuntime({ check }));

    updater.start();

    await vi.waitFor(() => expect(check).toHaveBeenCalledTimes(1));
    expect(updater.state.value).toEqual({ phase: "idle" });
  });

  it("retries a failed check on the next six-hour tick", async () => {
    const check = vi
      .fn<UpdaterRuntime["check"]>()
      .mockRejectedValueOnce(new Error("synthetic outage"))
      .mockResolvedValue(null);
    const updater = useUpdater(makeRuntime({ check }));

    updater.start();
    await vi.waitFor(() =>
      expect(updater.state.value).toEqual({
        phase: "failed",
        stage: "check"
      })
    );

    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);

    expect(check).toHaveBeenCalledTimes(2);
    expect(updater.state.value).toEqual({ phase: "idle" });
  });

  it("does not recheck while a verified update is ready", async () => {
    const update = makeCandidate();
    const check = vi
      .fn<UpdaterRuntime["check"]>()
      .mockResolvedValue(update.candidate);
    const updater = useUpdater(makeRuntime({ check }));

    updater.start();
    await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
    await vi.advanceTimersByTimeAsync(UPDATE_CHECK_INTERVAL_MS);

    expect(check).toHaveBeenCalledTimes(1);
    expect(updater.state.value).toEqual({
      phase: "ready",
      version: "0.4.0"
    });
  });

  it("reports known progress and becomes ready only after download resolves", async () => {
    const downloadGate = deferred<void>();
    const update = makeCandidate("0.4.0", downloadGate);
    const updater = useUpdater(
      makeRuntime({ check: vi.fn().mockResolvedValue(update.candidate) })
    );

    updater.start();
    await vi.waitFor(() =>
      expect(updater.state.value.phase).toBe("downloading")
    );
    update.emit({ event: "Started", data: { contentLength: 200 } });
    update.emit({ event: "Progress", data: { chunkLength: 84 } });

    expect(updater.state.value).toEqual({
      phase: "downloading",
      version: "0.4.0",
      downloaded: 84,
      total: 200,
      percent: 42
    });

    downloadGate.resolve();
    await vi.waitFor(() =>
      expect(updater.state.value).toEqual({
        phase: "ready",
        version: "0.4.0"
      })
    );
  });

  it("keeps progress indeterminate when content length is unavailable", async () => {
    const downloadGate = deferred<void>();
    const update = makeCandidate("0.4.0", downloadGate);
    const updater = useUpdater(
      makeRuntime({ check: vi.fn().mockResolvedValue(update.candidate) })
    );

    updater.start();
    await vi.waitFor(() =>
      expect(updater.state.value.phase).toBe("downloading")
    );
    update.emit({ event: "Started", data: {} });
    update.emit({ event: "Progress", data: { chunkLength: 32 } });

    expect(updater.state.value).toEqual({
      phase: "downloading",
      version: "0.4.0",
      downloaded: 32
    });
    downloadGate.resolve();
  });

  it("keeps a verified update ready when confirmation is cancelled", async () => {
    const update = makeCandidate();
    const runtime = makeRuntime({
      check: vi.fn().mockResolvedValue(update.candidate),
      confirm: vi.fn().mockResolvedValue(false)
    });
    const updater = useUpdater(runtime);

    updater.start();
    await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
    await updater.activate(confirmation);

    expect(runtime.confirm).toHaveBeenCalledWith(confirmation.message, {
      title: confirmation.title,
      kind: "info"
    });
    expect(update.candidate.install).not.toHaveBeenCalled();
    expect(update.candidate.close).not.toHaveBeenCalled();
    expect(updater.state.value).toEqual({
      phase: "ready",
      version: "0.4.0"
    });
  });

  it("installs with the plugin's zero-argument contract and relaunches when install returns", async () => {
    const update = makeCandidate();
    const runtime = makeRuntime({
      check: vi.fn().mockResolvedValue(update.candidate)
    });
    const updater = useUpdater(runtime);

    updater.start();
    await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
    await updater.activate(confirmation);

    expect(vi.mocked(update.candidate.install).mock.calls).toEqual([[]]);
    expect(update.candidate.close).toHaveBeenCalledTimes(1);
    expect(runtime.relaunch).toHaveBeenCalledTimes(1);
    expect(updater.state.value).toEqual({
      phase: "installing",
      version: "0.4.0"
    });
  });

  it("closes a failed download and retries from a fresh check", async () => {
    const broken = makeCandidate();
    vi.mocked(broken.candidate.download).mockRejectedValue(
      new Error("synthetic download failure")
    );
    const recovered = makeCandidate("0.4.1");
    const check = vi
      .fn<UpdaterRuntime["check"]>()
      .mockResolvedValueOnce(broken.candidate)
      .mockResolvedValueOnce(recovered.candidate);
    const updater = useUpdater(makeRuntime({ check }));

    updater.start();
    await vi.waitFor(() =>
      expect(updater.state.value).toEqual({
        phase: "failed",
        stage: "download"
      })
    );
    expect(broken.candidate.close).toHaveBeenCalledTimes(1);

    await updater.activate(confirmation);
    await vi.waitFor(() =>
      expect(updater.state.value).toEqual({
        phase: "ready",
        version: "0.4.1"
      })
    );
    expect(check).toHaveBeenCalledTimes(2);
  });

  it.each([
    ["check", "check"],
    ["confirm", "confirm"],
    ["install", "install"],
    ["relaunch", "relaunch"]
  ] as const)(
    "records a retryable %s failure without exposing its error",
    async (operation, stage) => {
      const update = makeCandidate();
      const runtime = makeRuntime({
        check:
          operation === "check"
            ? vi.fn().mockRejectedValue(new Error("private check detail"))
            : vi.fn().mockResolvedValue(update.candidate),
        confirm:
          operation === "confirm"
            ? vi.fn().mockRejectedValue(new Error("private dialog detail"))
            : vi.fn().mockResolvedValue(true),
        relaunch:
          operation === "relaunch"
            ? vi.fn().mockRejectedValue(new Error("private relaunch detail"))
            : vi.fn().mockResolvedValue(undefined)
      });
      if (operation === "install") {
        vi.mocked(update.candidate.install).mockRejectedValue(
          new Error("private installer detail")
        );
      }
      const updater = useUpdater(runtime);

      updater.start();
      if (operation !== "check") {
        await vi.waitFor(() =>
          expect(updater.state.value.phase).toBe("ready")
        );
        await updater.activate(confirmation);
      }

      await vi.waitFor(() =>
        expect(updater.state.value).toEqual({ phase: "failed", stage })
      );
      expect(JSON.stringify(updater.state.value)).not.toContain("private");
    }
  );

  it("closes a retained ready update when stopped", async () => {
    const update = makeCandidate();
    const updater = useUpdater(
      makeRuntime({ check: vi.fn().mockResolvedValue(update.candidate) })
    );

    updater.start();
    await vi.waitFor(() => expect(updater.state.value.phase).toBe("ready"));
    updater.stop();

    await vi.waitFor(() =>
      expect(update.candidate.close).toHaveBeenCalledTimes(1)
    );
  });
});
