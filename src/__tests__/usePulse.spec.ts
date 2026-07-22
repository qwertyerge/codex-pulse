import { beforeEach, describe, expect, it, vi } from "vitest";

const { invoke } = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { usePulse } from "../composables/usePulse";

describe("usePulse", () => {
  beforeEach(() => invoke.mockReset());

  it("loads the initial native snapshot", async () => {
    invoke.mockResolvedValueOnce({
      sessions: [
        {
          threadId: "00000000-0000-4000-8000-000000000001",
          title: "Root task",
          cwd: "/repo",
          sessionCreatedAtMs: 1_000,
          currentRunStartedAtMs: 2_000
        }
      ],
      isLoading: false,
      initialization: { phase: "idle", events: [] },
      monitoring: { enabled: false, needsRepair: false, staleCount: 0 },
      alwaysOnTop: false,
      launchAtLogin: false,
      locale: "system",
      theme: "system"
    });

    const pulse = usePulse();
    await pulse.load();

    expect(invoke).toHaveBeenCalledWith("get_snapshot");
    expect(pulse.snapshot.value.sessions[0]?.title).toBe("Root task");
    expect(pulse.snapshot.value.weeklyQuota).toBeUndefined();
    expect(pulse.snapshot.value.theme).toBe("system");
    expect(pulse.snapshot.value.initialization.events).toEqual([]);
  });

  it("rolls back Pin to Top when the native command fails", async () => {
    invoke.mockResolvedValueOnce({
      sessions: [],
      isLoading: false,
      monitoring: { enabled: false, needsRepair: false, staleCount: 0 },
      alwaysOnTop: false,
      launchAtLogin: false,
      locale: "system"
    });
    const pulse = usePulse();
    await pulse.load();
    invoke.mockRejectedValueOnce(new Error("window is unavailable"));

    await pulse.togglePin();

    expect(invoke).toHaveBeenLastCalledWith("set_always_on_top", { value: true });
    expect(pulse.snapshot.value.alwaysOnTop).toBe(false);
    expect(pulse.error.value).toContain("window is unavailable");
  });

  it("delegates a card click to the validated native deep-link command", async () => {
    const pulse = usePulse();
    invoke.mockResolvedValueOnce(undefined);

    await pulse.openThread("00000000-0000-4000-8000-000000000001");

    expect(invoke).toHaveBeenCalledWith("open_thread", {
      threadId: "00000000-0000-4000-8000-000000000001"
    });
  });

  it("opens a project through the validated native path command", async () => {
    const pulse = usePulse();
    invoke.mockResolvedValueOnce(undefined);

    await pulse.openProjectPath("/workspace/codex-pulse");

    expect(invoke).toHaveBeenCalledWith("open_project_path", {
      path: "/workspace/codex-pulse"
    });
    expect(pulse.error.value).toBeUndefined();
  });

  it("surfaces a native project-open failure", async () => {
    const pulse = usePulse();
    invoke.mockRejectedValueOnce(new Error("Project path is not a directory"));

    await pulse.openProjectPath("/workspace/file.txt");

    expect(pulse.error.value).toBe("Project path is not a directory");
  });

  it("persists an explicit theme and updates the local snapshot", async () => {
    const pulse = usePulse();
    invoke.mockResolvedValueOnce("dark");

    await pulse.setTheme("dark");

    expect(invoke).toHaveBeenLastCalledWith("set_theme", { theme: "dark" });
    expect(pulse.snapshot.value.theme).toBe("dark");
  });

  it("rolls back a locale when native persistence fails", async () => {
    const pulse = usePulse();
    invoke.mockRejectedValueOnce(new Error("config unavailable"));

    await pulse.setLocale("fr");

    expect(invoke).toHaveBeenLastCalledWith("set_locale", { locale: "fr" });
    expect(pulse.snapshot.value.locale).toBe("system");
    expect(pulse.error.value).toContain("config unavailable");
  });

  it("keeps initialization streams isolated by run and ignores delayed prior events", () => {
    const pulse = usePulse();

    pulse.mergeInitializationEvent({ runId: 1, sequence: 1, occurredAtMs: 1, phase: "starting", summary: "first" });
    pulse.mergeInitializationEvent({ runId: 2, sequence: 1, occurredAtMs: 2, phase: "starting", summary: "second" });
    pulse.mergeInitializationEvent({ runId: 1, sequence: 2, occurredAtMs: 3, phase: "complete", summary: "late first" });

    expect(pulse.snapshot.value.initialization.runId).toBe(2);
    expect(pulse.snapshot.value.initialization.events.map((event) => event.summary)).toEqual(["second"]);
  });
});
