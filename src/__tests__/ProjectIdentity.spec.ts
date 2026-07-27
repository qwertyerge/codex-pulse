import { enableAutoUnmount, mount } from "@vue/test-utils";
import { nextTick } from "vue";
import { afterEach, describe, expect, it, vi } from "vitest";
import ProjectIdentity from "../components/ProjectIdentity.vue";
import { i18n } from "../i18n";
import "../styles.css";

const originalInnerHeight = window.innerHeight;
enableAutoUnmount(afterEach);

const git = {
  projectName: "codex-pulse",
  primaryCheckoutPath: "/src/codex-pulse",
  branch: "feature/git-context",
  defaultBranch: "trunk",
  defaultUpstream: "company/trunk",
  remoteUrl: "https://example.com/acme/codex-pulse.git"
};

afterEach(() => {
  i18n.global.locale.value = "en";
  Object.defineProperty(window, "innerHeight", {
    configurable: true,
    value: originalInnerHeight
  });
  vi.restoreAllMocks();
});

describe("ProjectIdentity", () => {
  it("renders git project and branch while opening the original cwd", async () => {
    const wrapper = mount(ProjectIdentity, {
      attachTo: document.body,
      props: { cwd: "/worktrees/9b55/codex-pulse", git },
      global: { plugins: [i18n] }
    });

    const link = wrapper.get("a.session-card__path");
    expect(link.text()).toBe("codex-pulse");
    expect(link.attributes("title")).toBeUndefined();
    expect(wrapper.get(".session-card__branch").text()).toContain("feature/git-context");

    await link.trigger("click");
    expect(wrapper.emitted("open-project")).toEqual([["/worktrees/9b55/codex-pulse"]]);
  });

  it("shows repository metadata on hover and keyboard focus", async () => {
    const wrapper = mount(ProjectIdentity, {
      attachTo: document.body,
      props: { cwd: "/worktrees/project", git },
      global: { plugins: [i18n] }
    });
    const link = wrapper.get("a.session-card__path");
    expect(link.attributes("aria-describedby")).toBeUndefined();

    await link.trigger("mouseenter");
    await nextTick();
    let popup = document.body.querySelector('[role="tooltip"]') as HTMLElement;
    expect(link.attributes("aria-describedby")).toBe(popup.id);
    expect(popup.textContent).toContain("Default branch");
    expect(popup.textContent).toContain("trunk");
    expect(popup.textContent).toContain("https://example.com/acme/codex-pulse.git");

    await link.trigger("mouseleave");
    await nextTick();
    expect(link.attributes("aria-describedby")).toBeUndefined();
    await link.trigger("focus");
    await nextTick();
    popup = document.body.querySelector('[role="tooltip"]') as HTMLElement;
    expect(popup).not.toBeNull();
    expect(link.attributes("aria-describedby")).toBe(popup.id);
  });

  it("keeps the popup open while either focus or hover remains active", async () => {
    const wrapper = mount(ProjectIdentity, {
      attachTo: document.body,
      props: { cwd: "/worktrees/project", git },
      global: { plugins: [i18n] }
    });
    const link = wrapper.get("a.session-card__path");

    await link.trigger("focus");
    await link.trigger("mouseenter");
    await link.trigger("mouseleave");
    await nextTick();
    expect(document.body.querySelector('[role="tooltip"]')).not.toBeNull();

    await link.trigger("blur");
    await nextTick();
    expect(document.body.querySelector('[role="tooltip"]')).toBeNull();
  });

  it("distinguishes detached HEAD from a non-Git directory", async () => {
    const detached = mount(ProjectIdentity, {
      attachTo: document.body,
      props: { cwd: "/worktrees/project", git: { ...git, branch: undefined } },
      global: { plugins: [i18n] }
    });
    expect(detached.get(".session-card__branch").text()).toContain("No branch");
    detached.unmount();

    const plain = mount(ProjectIdentity, {
      attachTo: document.body,
      props: { cwd: "/tmp/plain-directory" },
      global: { plugins: [i18n] }
    });
    expect(plain.get(".session-card__path").text()).toBe("plain-directory");
    expect(plain.get(".session-card__path").attributes("title")).toBeUndefined();
    expect(plain.find(".session-card__branch").exists()).toBe(false);
    await plain.get(".session-card__path").trigger("mouseenter");
    expect(document.body.querySelector('[role="tooltip"]')).toBeNull();
  });

  it("labels unavailable repository fields as not configured", async () => {
    const wrapper = mount(ProjectIdentity, {
      attachTo: document.body,
      props: {
        cwd: "/worktrees/project",
        git: { ...git, defaultBranch: undefined, remoteUrl: undefined }
      },
      global: { plugins: [i18n] }
    });

    await wrapper.get(".session-card__path").trigger("mouseenter");
    await nextTick();
    const popup = document.body.querySelector('[role="tooltip"]') as HTMLElement;
    expect(popup.textContent?.match(/Not configured/g)).toHaveLength(2);
  });

  it("places the hover card above when it does not fit below", async () => {
    const wrapper = mount(ProjectIdentity, {
      attachTo: document.body,
      props: { cwd: "/worktrees/project", git },
      global: { plugins: [i18n] }
    });
    const link = wrapper.get("a.session-card__path");
    vi.spyOn(link.element, "getBoundingClientRect").mockReturnValue({
      x: 16,
      y: 180,
      top: 180,
      right: 140,
      bottom: 200,
      left: 16,
      width: 124,
      height: 20,
      toJSON: () => ({})
    });
    Object.defineProperty(window, "innerHeight", {
      configurable: true,
      value: 240
    });

    await link.trigger("mouseenter");
    await nextTick();
    expect(document.body.querySelector('[role="tooltip"]')?.getAttribute("data-placement"))
      .toBe("above");
  });

  it("applies compact identity layout and an unclipped fixed hover card", async () => {
    const wrapper = mount(ProjectIdentity, {
      attachTo: document.body,
      props: { cwd: "/worktrees/project", git },
      global: { plugins: [i18n] }
    });

    const project = getComputedStyle(wrapper.get(".session-card__project").element);
    const branch = getComputedStyle(wrapper.get(".session-card__branch").element);
    expect(project.display).toBe("flex");
    expect(project.minWidth).toBe("0px");
    expect(branch.maxWidth).toBe("48%");

    await wrapper.get(".session-card__path").trigger("mouseenter");
    await nextTick();
    const popup = document.body.querySelector('[role="tooltip"]') as HTMLElement;
    const popupStyle = getComputedStyle(popup);
    expect(popupStyle.position).toBe("fixed");
    expect(popupStyle.zIndex).toBe("20");
    expect(popupStyle.pointerEvents).toBe("none");
  });
});
