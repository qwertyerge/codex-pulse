import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";
import { parse } from "yaml";

interface IssueForm {
  name: string;
  description: string;
  title: string;
  labels: string[];
  body: Array<{ id?: string; type: string }>;
}

function readRoot(path: string) {
  const absolute = resolve(process.cwd(), path);
  expect(existsSync(absolute), `${path} should exist`).toBe(true);
  return readFileSync(absolute, "utf8");
}

function readIssueForm(name: string) {
  return parse(readRoot(`.github/ISSUE_TEMPLATE/${name}`)) as IssueForm;
}

describe("GitHub community configuration", () => {
  it("declares Apache-2.0 consistently", () => {
    const license = readRoot("LICENSE");
    const packageJson = JSON.parse(readRoot("package.json")) as {
      license: string;
      repository: { type: string; url: string };
    };
    const cargo = readRoot("src-tauri/Cargo.toml");

    expect(license).toContain(
      "Apache License\n                           Version 2.0, January 2004",
    );
    expect(license).toContain("http://www.apache.org/licenses/");
    expect(packageJson.license).toBe("Apache-2.0");
    expect(packageJson.repository).toEqual({
      type: "git",
      url: "https://github.com/qwertyerge/codex-pulse.git",
    });
    expect(cargo).toContain('license = "Apache-2.0"');
    expect(cargo).toContain(
      'repository = "https://github.com/qwertyerge/codex-pulse"',
    );
  });

  it("keeps the English and Chinese public identity aligned", () => {
    const english = readRoot("README.md");
    const chinese = readRoot("docs/README.zh-CN.md");

    for (const badge of [
      "actions/workflows/ci.yml/badge.svg",
      "github/v/release",
      "github/license",
    ]) {
      expect(english).toContain(badge);
      expect(chinese).toContain(badge);
    }
    expect(english).toContain("independent community project");
    expect(english).toContain("not affiliated with or endorsed by OpenAI");
    expect(english).toContain("not Developer ID signed or Apple notarized");
    expect(english).toContain("Windows 11 x64");
    expect(english).toContain("native Codex");
    expect(english).toMatch(/WSL.*Unsupported/);
    expect(english).toContain("unsigned");
    expect(english).toContain("pending-user-eyeball");
    expect(english).toContain("## Build from Source");
    expect(english).toContain("## License");
    expect(chinese).toContain("独立社区项目");
    expect(chinese).toContain("与 OpenAI 无隶属关系，也未获得其认可");
    expect(chinese).toContain("未使用 Developer ID 签名，也未经过 Apple 公证");
    expect(chinese).toContain("Windows 11 x64");
    expect(chinese).toContain("原生 Codex");
    expect(chinese).toMatch(/WSL.*不支持/);
    expect(chinese).toContain("未签名");
    expect(chinese).toContain("pending-user-eyeball");
    expect(chinese).toContain("## 从源码构建");
    expect(chinese).toContain("## 许可证");
  });

  it("provides contribution, security, issue, and pull-request guidance", () => {
    const contributing = readRoot("CONTRIBUTING.md");
    const security = readRoot("SECURITY.md");
    const pullRequest = readRoot(".github/pull_request_template.md");
    const config = parse(
      readRoot(".github/ISSUE_TEMPLATE/config.yml"),
    ) as {
      blank_issues_enabled: boolean;
    };
    const bug = readIssueForm("bug_report.yml");
    const feature = readIssueForm("feature_request.yml");

    expect(contributing).toContain("pnpm test");
    expect(contributing).toContain(
      "cargo test --manifest-path src-tauri/Cargo.toml",
    );
    expect(contributing).toContain("Rust (Windows)");
    expect(contributing).toContain(
      "pnpm tauri build --target x86_64-pc-windows-msvc --bundles nsis",
    );
    expect(contributing).toContain("Do not include local Codex transcripts");
    expect(security).toContain("public GitHub issue");
    expect(security).toContain(
      "does not currently offer a private reporting channel",
    );
    expect(security).toContain("operating system and version");
    expect(security).toContain("architecture");
    expect(security).toContain("Codex environment");
    expect(pullRequest).toContain("Privacy checklist");
    expect(config.blank_issues_enabled).toBe(false);
    expect(bug.labels).toEqual(["bug"]);
    expect(bug.body.map((item) => item.id).filter(Boolean)).toEqual(
      [
        "version",
        "operating_system",
        "architecture",
        "codex_environment",
        "problem",
        "steps",
        "expected",
        "actual",
        "logs",
        "privacy",
      ],
    );
    expect(feature.labels).toEqual(["enhancement"]);
    expect(feature.body.map((item) => item.id).filter(Boolean)).toEqual(
      expect.arrayContaining([
        "problem",
        "outcome",
        "alternatives",
        "context",
        "privacy",
      ]),
    );
  });
});
