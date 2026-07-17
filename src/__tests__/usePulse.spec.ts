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
      monitoring: { enabled: false, needsRepair: false, staleCount: 0 },
      alwaysOnTop: false,
      launchAtLogin: false,
      locale: "system"
    });

    const pulse = usePulse();
    await pulse.load();

    expect(invoke).toHaveBeenCalledWith("get_snapshot");
    expect(pulse.snapshot.value.sessions[0]?.title).toBe("Root task");
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
});
