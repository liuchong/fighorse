# Code Connect Contract

This document defines fighorse's native Code Connect compatibility boundary.

## Scope

fighorse supports modern Figma Code Connect template workflows without requiring
Node.js, the official Code Connect CLI, or the official Figma MCP server.

The initial contract covers:

- Parserless `.figma.ts`, `.figma.js`, `.figma.template.ts`,
  `.figma.template.js`, and `.figma.batch.json` files.
- AI-assisted template generation from a Figma component URL plus explicit code
  component context supplied by the calling AI client or user.
- Local parsing and validation that never executes template code.
- Remote validation, preview, publish, and unpublish through Figma's observed
  Code Connect service protocol.

The implementation compatibility baseline is the official `figma/code-connect`
repository at commit `d856baa9bf7907c430121139725a7a8181bac9a3`, CLI version
`2.1.0`, observed on 2026-11-19.

Key `v2.1.0` capabilities:

- Sends `User-Agent: code-connect-cli/2.1.0 (@figma/code-connect)` to negotiate
  structured server responses (`meta.failed_nodes[].errors` with
  `CODE_CONNECT_PRO_LIBRARY_INACTIVE` / `CODE_CONNECT_PRO_FRAMEWORK_INACTIVE`,
  and `meta.new_active_selections`).
- Supports `SLOT` component property generation
  (`figma.selectedInstance.getSlot(...)`), trailing `//` comments on
  `import figma from 'figma'`, leading `/` in `include` globs, single-template
  preview resolution against `.figma.batch.json`, `--unique` snippet
  deduplication, and clear migration errors on legacy `figma.connect(...)`
  parser files.

## Protocol Boundary

The following endpoints are observed from the official open-source client, but
are not part of fighorse's public REST OpenAPI coverage snapshot:

- `GET /v1/code_connect/{file_key}/cli_data`
- `POST /v1/code_connect/preview_snippets`
- `POST /v1/code_connect`
- `DELETE /v1/code_connect`

fighorse treats these endpoints as an `observed-private-protocol`. If Figma
changes them incompatibly, fighorse must report `protocol_incompatible` with the
phase that failed and the compatibility baseline. It must not silently degrade
to Dev Resources links or claim a successful Code Connect publish.

## Persistent State

The only persistent project state is the user's template files and optional
`figma.config.json`. fighorse does not keep a local mapping database and does
not assume Figma exposes a supported API for listing all remote Code Connect
mappings.

## Security Rules

- Figma tokens are read from `FIGMA_TOKEN`, `FIGMA_ACCESS_TOKEN`,
  `FIGMA_API_KEY`, or the user's fighorse config. Tokens are never written to
  generated templates, logs, fixtures, or manifests.
- Project-owned `figma.config.json` may control include/exclude, label,
  language, and URL substitutions. fighorse rejects project-supplied `apiUrl`
  so a repository cannot redirect a token to a third-party host.
- Template parsing and generation do not send code to Figma. Preview sends
  template code to Figma for real server-side rendering. Publish mutates Figma
  Dev Mode mappings. MCP preview/publish therefore require explicit Code
  Connect egress permission; publish and unpublish also require Figma write
  mode.
- Template code is never executed locally.

## Non-Goals

The initial contract does not implement legacy React, HTML, Storybook, SwiftUI,
or Jetpack Compose parsers. It does not implement Figma's full template runtime
locally and does not provide offline preview.

Official automatic mapping discovery remains a Figma product capability. The
fighorse generator performs deterministic assisted mapping from explicit input
and must mark ambiguous properties instead of guessing.
