#!/usr/bin/env bash

set -euo pipefail

if [[ -z "${GITHUB_TOKEN:-}" ]]; then
  printf '%s\n' "GITHUB_TOKEN is required" >&2
  exit 1
fi

printf 'x-access-token:%s' "$GITHUB_TOKEN" | base64 | tr -d '\r\n'
