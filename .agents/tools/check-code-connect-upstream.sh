#!/usr/bin/env bash
set -euo pipefail

repo="figma/code-connect"
baseline="d856baa9bf7907c430121139725a7a8181bac9a3"

current="$(gh api "repos/${repo}/commits/main" --jq '.sha')"
latest_release="$(gh release view --repo "${repo}" --json tagName,publishedAt --jq '.tagName + " " + .publishedAt')"

printf 'repo=%s\n' "${repo}"
printf 'baseline=%s\n' "${baseline}"
printf 'main=%s\n' "${current}"
printf 'latest_release=%s\n' "${latest_release}"

if [ "${current}" != "${baseline}" ]; then
  printf 'status=upstream-drift\n'
  printf 'next_step=Review cli/src/connect and cli/src/commands before changing fighorse Code Connect behavior.\n'
else
  printf 'status=baseline-current\n'
fi
