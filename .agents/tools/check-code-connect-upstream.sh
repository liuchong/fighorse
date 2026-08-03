#!/usr/bin/env bash
set -euo pipefail

repo="figma/code-connect"
baseline="6a6b50b1f71438768512e1b67475ba2bd555a018"

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
