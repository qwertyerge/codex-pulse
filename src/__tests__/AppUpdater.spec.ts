import { flushPromises, mount } from "@vue/test-utils";
import {
  afterEach,
  beforeEach,
  describe,
  expect,
  it,
  vi
} from "vitest";
import type { AppSnapshot } from "../types";

const boundary = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
  check: vi.fn(),
  confirm: vi.fn(),
  relaunch: vi.fn()
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: boundary.invoke,
  isTauri: () => true
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: boundary.listen
}));
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: boundary.check
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  confirm: boundary.confirm
}));
vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: boundary.relaunch
}));

function deferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function snapshot(): AppSnapshot {
  return {
    sessions: [],
    weeklyQuota: undefined,
    isLoading: false,
    initialization: { runId: 1, phase: "complete", events: [] },
    monitoring: {
      enabled: true,
      needsRepair: false,
      staleCount: 0
    },
    alwaysOnTop: false,
    launchAtLogin: false,
    locale: "en",
    theme: "system"
  };
}

async function mountApp() {
  const [{ default: App }, { i18n }] = await Promise.all([
    import("../App.vue"),
    import("../i18n")
  ]);
  return mount(App, { global: { plugins: [i18n] } });
}

describe("App automatic updater integration", () => {
  beforeEach(() => {
    vi.resetModules();
    vi.useFakeTimers();
    boundary.invoke.mockReset();
    boundary.listen.mockReset().mockResolvedValue(() => undefined);
    boundary.check.mockReset().mockResolvedValue(null);
    boundary.confirm.mockReset().mockResolvedValue(true);
    boundary.relaunch.mockReset().mockResolvedValue(undefined);
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    vi.useRealTimers();
  });

  it("waits for the first snapshot, checks in production, and stops on unmount", async () => {
    const firstSnapshot = deferred<AppSnapshot>();
    boundary.invoke.mockImplementation((command: string) => {
      if (command === "get_snapshot") return firstSnapshot.promise;
      return Promise.resolve(undefined);
    });
    vi.stubEnv("PROD", true);
    const wrapper = await mountApp();

    await flushPromises();
    expect(boundary.check).not.toHaveBeenCalled();

    firstSnapshot.resolve(snapshot());
    await flushPromises();
    expect(boundary.check).toHaveBeenCalledTimes(1);

    wrapper.unmount();
    await vi.advanceTimersByTimeAsync(21_600_000);
    expect(boundary.check).toHaveBeenCalledTimes(1);
  });

  it("does not contact the updater outside a production build", async () => {
    boundary.invoke.mockResolvedValue(snapshot());
    vi.stubEnv("PROD", false);
    const wrapper = await mountApp();

    await flushPromises();

    expect(boundary.check).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("renders a ready update and sends localized copy to the native dialog", async () => {
    const update = {
      version: "0.4.0",
      download: vi.fn().mockResolvedValue(undefined),
      install: vi.fn().mockResolvedValue(undefined),
      close: vi.fn().mockResolvedValue(undefined)
    };
    boundary.invoke.mockResolvedValue(snapshot());
    boundary.check.mockResolvedValue(update);
    vi.stubEnv("PROD", true);
    const wrapper = await mountApp();

    await vi.waitFor(() =>
      expect(wrapper.get(".top-bar__update").text()).toBe("Update")
    );
    await wrapper.get(".top-bar__update").trigger("click");
    await flushPromises();

    expect(boundary.confirm).toHaveBeenCalledWith(
      "Version 0.4.0 is ready. Install it and restart Codex Pulse?",
      { title: "Install Codex Pulse update", kind: "info" }
    );
    expect(update.install).toHaveBeenCalledWith({
      restartAfterInstall: true
    });
    expect(boundary.relaunch).toHaveBeenCalledTimes(1);
    wrapper.unmount();
  });
});
