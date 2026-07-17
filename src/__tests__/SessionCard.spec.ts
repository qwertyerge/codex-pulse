import { mount } from "@vue/test-utils";
import { beforeEach, describe, expect, it } from "vitest";
import SessionCard from "../components/SessionCard.vue";
import { i18n } from "../i18n";

describe("SessionCard", () => {
  beforeEach(() => { i18n.global.locale.value = "en"; });

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
      },
      global: { plugins: [i18n] }
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
      },
      global: { plugins: [i18n] }
    });

    expect(wrapper.get(".session-card__recent").text()).toContain("Completed cargo test");
  });

  it("opens Codex only from the dedicated Open icon action", async () => {
    const wrapper = mount(SessionCard, {
      props: {
        session: {
          threadId: "00000000-0000-4000-8000-000000000001",
          title: "Implement session monitor",
          cwd: "/repo",
          sessionCreatedAtMs: 1_000,
          currentRunStartedAtMs: 61_000
        },
        nowMs: 121_000
      },
      global: { plugins: [i18n] }
    });

    expect(wrapper.get(".session-card__main").element.tagName).not.toBe("BUTTON");
    await wrapper.get(".session-card__main").trigger("click");
    expect(wrapper.emitted("open")).toBeUndefined();

    await wrapper.get(".session-card__open").trigger("click");
    expect(wrapper.emitted("open")).toEqual([["00000000-0000-4000-8000-000000000001"]]);
    expect(wrapper.get(".session-card__open").attributes("aria-label")).toContain("Open Codex task");
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
    const wrapper = mount(SessionCard, { props: { session: initialSession, nowMs: 121_000 }, global: { plugins: [i18n] } });

    await wrapper.get(".session-card__recent-toggle").trigger("click");
    expect(wrapper.get(".session-card__recent-detail").text()).toContain("verified the runtime state");
    expect(wrapper.get(".session-card__recent-age-value").text()).toBe("21s");
    expect(wrapper.get(".session-card__recent-paused").attributes("aria-label")).toBe("Recent age paused");

    await wrapper.setProps({
      nowMs: 181_000,
      session: {
        ...initialSession,
        recentEvent: { summary: "Updated styles", detail: "Updated styles", occurredAtMs: 121_000 }
      }
    });
    expect(wrapper.get(".session-card__recent-detail").text()).toContain("verified the runtime state");
    expect(wrapper.get(".session-card__recent-age-value").text()).toBe("21s");

    await wrapper.get(".session-card__recent-toggle").trigger("click");
    expect(wrapper.text()).toContain("Updated styles");
    expect(wrapper.find(".session-card__recent-paused").exists()).toBe(false);
  });

  it("renders expanded prompt and recent detail as sanitized Markdown with a fixed ago suffix", async () => {
    const wrapper = mount(SessionCard, {
      props: {
        session: {
          threadId: "00000000-0000-4000-8000-000000000001",
          title: "Implement session monitor",
          cwd: "/repo",
          sessionCreatedAtMs: 1_000,
          currentRunStartedAtMs: 61_000,
          lastUserMessage: { content: "**Prompt**\n\n- first item\n- second item\n\n<script>alert(1)</script>", occurredAtMs: 100_000 },
          recentEvent: { summary: "**Recent**", detail: "**Recent detail**", occurredAtMs: 100_000 }
        },
        nowMs: 121_000
      },
      global: { plugins: [i18n] }
    });

    expect(wrapper.get(".session-card__recent-age-value").text()).toBe("21s");
    expect(wrapper.get(".session-card__recent-age-suffix").text().trim()).toBe("ago");
    await wrapper.findAll(".session-card__meta-row")[0].trigger("click");
    expect(wrapper.get(".markdown-content").html()).toContain("<strong>Prompt</strong>");
    expect(wrapper.get(".markdown-content").html()).toContain("<ul>");
    expect(wrapper.get(".markdown-content").html()).not.toContain("<script");
    await wrapper.findAll(".session-card__meta-row")[1].trigger("click");
    expect(wrapper.findAll(".markdown-content")[1].html()).toContain("<strong>Recent detail</strong>");
  });

  it("uses the selected locale for task and timer labels", () => {
    i18n.global.locale.value = "de";
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
      },
      global: { plugins: [i18n] }
    });

    expect(wrapper.get("button").attributes("aria-label")).toContain("Codex-Aufgabe öffnen");
    expect(wrapper.text()).toContain("Aktueller Lauf");
  });
});
