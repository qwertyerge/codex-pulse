import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SessionCard from "../components/SessionCard.vue";

describe("SessionCard", () => {
  it("shows title, complete path tooltip, and both timers", () => {
    const wrapper = mount(SessionCard, {
      props: {
        session: {
          threadId: "00000000-0000-4000-8000-000000000001",
          title: "Implement session monitor",
          cwd: "/workspace/project",
          sessionCreatedAtMs: 1_000,
          currentRunStartedAtMs: 61_000
        },
        nowMs: 121_000
      }
    });

    expect(wrapper.text()).toContain("Implement session monitor");
    expect(wrapper.text()).toContain("Current run");
    expect(wrapper.text()).toContain("01:00");
    expect(wrapper.text()).toContain("Session age");
    expect(wrapper.text()).toContain("02:00");
    expect(wrapper.get(".session-card__path").attributes("title")).toBe("/workspace/project");
    expect(wrapper.get("button").attributes("aria-label")).toContain("Open Codex task");
  });

  it("shows only the latest meaningful event below the timers", () => {
    const wrapper = mount(SessionCard, {
      props: {
        session: {
          threadId: "00000000-0000-4000-8000-000000000001",
          title: "Implement session monitor",
          cwd: "/repo",
          sessionCreatedAtMs: 1_000,
          currentRunStartedAtMs: 61_000,
          recentEvent: { summary: "Completed cargo test", occurredAtMs: 100_000 }
        },
        nowMs: 121_000
      }
    });

    expect(wrapper.get(".session-card__recent").text()).toContain("Completed cargo test");
  });

  it("freezes the expanded event until it is collapsed", async () => {
    const initialSession = {
      threadId: "00000000-0000-4000-8000-000000000001",
      title: "Implement session monitor",
      cwd: "/repo",
      sessionCreatedAtMs: 1_000,
      currentRunStartedAtMs: 61_000,
      recentEvent: {
        summary: "Implemented monitor",
        detail: "Implemented monitor and verified the runtime state.",
        occurredAtMs: 100_000
      }
    };
    const wrapper = mount(SessionCard, { props: { session: initialSession, nowMs: 121_000 } });

    await wrapper.get(".session-card__recent-toggle").trigger("click");
    expect(wrapper.get(".session-card__recent-detail").text()).toContain("verified the runtime state");

    await wrapper.setProps({
      session: {
        ...initialSession,
        recentEvent: { summary: "Updated styles", detail: "Updated styles", occurredAtMs: 121_000 }
      }
    });
    expect(wrapper.get(".session-card__recent-detail").text()).toContain("verified the runtime state");

    await wrapper.get(".session-card__recent-toggle").trigger("click");
    expect(wrapper.text()).toContain("Updated styles");
  });
});
