import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { describe, expect, it } from "vitest";

interface RecoveryEvidence {
  schema_version: number;
  verified_at_utc: string;
  source_commit: string;
  tauri_cli_version: string;
  updater_plugin_version: string;
  minisign_verify_version: string;
  fixture_sha256: string;
  signature_sha256: string;
  public_key_config_sha256: string;
  encrypted_key_format_valid: boolean;
  independent_key_source_attested: boolean;
  separate_passphrase_source_attested: boolean;
  signature_verified: boolean;
  tampered_fixture_rejected: boolean;
}

const repositoryRoot = process.cwd();
const evidenceDirectory = resolve(
  repositoryRoot,
  "docs/superpowers/reports/0.4.0-updater-backup-recovery",
);

function read(name: string) {
  return readFileSync(join(evidenceDirectory, name));
}

function sha256(contents: Buffer | string) {
  return createHash("sha256").update(contents).digest("hex");
}

describe("updater signing backup recovery evidence", () => {
  it("contains only the approved schema and public facts", () => {
    const fixture = read("fixture.txt").toString("utf8");
    const signature = read("fixture.txt.sig").toString("utf8");
    const evidence = JSON.parse(
      read("verification.json").toString("utf8"),
    ) as RecoveryEvidence;

    expect(Object.keys(evidence).sort()).toEqual([
      "encrypted_key_format_valid",
      "fixture_sha256",
      "independent_key_source_attested",
      "minisign_verify_version",
      "public_key_config_sha256",
      "schema_version",
      "separate_passphrase_source_attested",
      "signature_sha256",
      "signature_verified",
      "source_commit",
      "tampered_fixture_rejected",
      "tauri_cli_version",
      "updater_plugin_version",
      "verified_at_utc",
    ]);
    expect(evidence).toMatchObject({
      schema_version: 1,
      tauri_cli_version: "tauri-cli 2.11.4",
      updater_plugin_version: "2.10.1",
      minisign_verify_version: "0.2.5",
      encrypted_key_format_valid: true,
      independent_key_source_attested: true,
      separate_passphrase_source_attested: true,
      signature_verified: true,
      tampered_fixture_rejected: true,
    });
    expect(evidence.verified_at_utc).toMatch(
      /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/,
    );
    expect(evidence.source_commit).toMatch(/^[0-9a-f]{40}$/);
    expect(fixture).toMatch(
      /^schema=codex-pulse-updater-backup-recovery-v1\nrepository=qwertyerge\/codex-pulse\nrelease=0\.4\.0\nsource_commit=[0-9a-f]{40}\nverified_at_utc=\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z\nnonce=[0-9a-f-]{36}\n$/,
    );
    expect(fixture).toContain(
      `source_commit=${evidence.source_commit}\n`,
    );
    expect(signature).toMatch(/^[A-Za-z0-9+/=]+$/);
    expect(
      Buffer.from(signature.trim(), "base64").toString("base64"),
    ).toBe(signature.trim());
    const decodedSignature = Buffer.from(
      signature.trim(),
      "base64",
    )
      .toString("utf8")
      .replace(/\r\n/g, "\n");
    const signatureLines = decodedSignature.endsWith("\n")
      ? decodedSignature.slice(0, -1).split("\n")
      : decodedSignature.split("\n");
    expect(signatureLines).toHaveLength(4);
    expect(signatureLines[0]).toBe(
      "untrusted comment: signature from tauri secret key",
    );
    expect(signatureLines[1]).toMatch(/^[A-Za-z0-9+/=]+$/);
    expect(signatureLines[2]).toMatch(
      /^trusted comment: timestamp:\d+\tfile:fixture\.txt$/,
    );
    expect(signatureLines[3]).toMatch(/^[A-Za-z0-9+/=]+$/);
    expect(sha256(read("fixture.txt"))).toBe(evidence.fixture_sha256);
    expect(sha256(read("fixture.txt.sig"))).toBe(
      evidence.signature_sha256,
    );

    const config = JSON.parse(
      readFileSync(
        resolve(repositoryRoot, "src-tauri/tauri.conf.json"),
        "utf8",
      ),
    ) as { plugins: { updater: { pubkey: string } } };
    expect(sha256(config.plugins.updater.pubkey)).toBe(
      evidence.public_key_config_sha256,
    );

    const publicText = [
      fixture,
      signature,
      decodedSignature,
      JSON.stringify(evidence),
    ].join("\n");
    expect(publicText).not.toMatch(
      /\/Users\/|\/Volumes\/|TAURI_SIGNING_PRIVATE_KEY|TAURI_PRIVATE_KEY|restored-updater\.key/,
    );
  });
});
