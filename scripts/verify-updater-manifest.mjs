#!/usr/bin/env node

import { readFile } from "node:fs/promises";

const [manifestPath, expectedVersion] = process.argv.slice(2);
const requiredPlatforms = ["darwin-aarch64", "windows-x86_64"];
const releaseDownloadPrefix =
  `/qwertyerge/codex-pulse/releases/download/${encodeURIComponent(expectedVersion)}/`;

function fail(message) {
  console.error(`Updater manifest invalid: ${message}`);
  process.exit(1);
}

if (!manifestPath || !expectedVersion) {
  fail("usage: verify-updater-manifest.mjs <path> <version>");
}

let manifest;
try {
  manifest = JSON.parse(await readFile(manifestPath, "utf8"));
} catch (error) {
  fail(
    `cannot read JSON: ${
      error instanceof Error ? error.message : String(error)
    }`
  );
}

if (
  typeof manifest !== "object" ||
  manifest === null ||
  Array.isArray(manifest)
) {
  fail("root must be an object");
}

if (manifest.version !== expectedVersion) {
  fail(
    `version ${String(manifest.version)} does not match tag ${expectedVersion}`
  );
}

if (
  typeof manifest.platforms !== "object" ||
  manifest.platforms === null ||
  Array.isArray(manifest.platforms)
) {
  fail("platforms must be an object");
}

for (const platform of requiredPlatforms) {
  const entry = manifest.platforms[platform];
  if (typeof entry !== "object" || entry === null || Array.isArray(entry)) {
    fail(`${platform} must be an object`);
  }
  for (const field of ["url", "signature"]) {
    if (typeof entry[field] !== "string" || entry[field].trim() === "") {
      fail(`${platform}.${field} must be non-empty`);
    }
  }

  let downloadUrl;
  try {
    downloadUrl = new URL(entry.url);
  } catch {
    fail(`${platform}.url must use the public GitHub release download URL`);
  }
  if (
    downloadUrl.origin !== "https://github.com" ||
    downloadUrl.username !== "" ||
    downloadUrl.password !== "" ||
    !downloadUrl.pathname.startsWith(releaseDownloadPrefix) ||
    downloadUrl.pathname === releaseDownloadPrefix ||
    downloadUrl.search !== "" ||
    downloadUrl.hash !== ""
  ) {
    fail(`${platform}.url must use the public GitHub release download URL`);
  }
}

console.log(
  `Validated updater manifest ${expectedVersion} for ${requiredPlatforms.join(", ")}`
);
