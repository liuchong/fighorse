---
name: fighorse
description: Recreate, inspect, export, and debug Figma designs with fighorse CLI or MCP. Use when the user asks to replicate a Figma design, inspect Figma nodes/tokens, export Figma assets, or connect an AI coding tool to Figma.
---

# fighorse

Use fighorse when a user asks to recreate, inspect, export, or debug a Figma design.

## Required User Setup

Figma API calls require a Figma Personal Access Token. Before calling Figma API tools, run `check_fighorse_ready` or `fighorse quickstart --format json`. If `auth.has_token` is false, do not keep trying Figma calls. Tell the user: `fighorse needs a Figma Personal Access Token. Run fighorse auth login --token <FIGMA_TOKEN> or set FIGMA_TOKEN, then retry.` Never ask the user to paste the token into chat unless they explicitly choose to; prefer local config or environment variables.

Native canvas writes do not use the public REST token path. They require the local plugin bridge: `fighorse install canvas-plugin --apply`, `fighorse canvas serve`, `fighorse canvas pair`, then running the imported fighorse Canvas Bridge plugin in Figma.

## Discovery

1. Call `fighorse discover --format json` or MCP `discover_fighorse` first.
2. Call `check_fighorse_ready` or `fighorse doctor --format json`; surface setup commands if auth/token is missing.
3. Call `fighorse experience summary --platform <platform> --asset-format <format>` or MCP `list_experiences` before implementation.
4. If platform or asset format is missing, ask the developer before choosing.
5. For exact public Figma REST API work, use `fighorse figma-api coverage --format json` or MCP resource `fighorse://coverage` to see the covered OpenAPI operations.
6. If an expected tool is missing after install, upgrade, or service restart, reconnect the MCP client and request `tools/list` again before deciding the feature is unavailable; clients can cache old tool lists for an active session.

## Replication

Use `get_design_package` or `fighorse design package <figma-url> --platform <platform> --asset-format <format>` as the main context source. Prioritize screenshots, learned_experience, explicit typography, tokens, compact tree metadata, then assets. If `scope.status=needs_narrowing` for a SECTION, CANVAS, DOCUMENT, or SELECTION, choose `screen_candidates[].id` where `implementable=true` and call `get_design_package` again; `diagnostics.screenshots.null_count` means the render URL was null and is not a usable screenshot.

## Official REST API

When the task needs a low-level Figma endpoint, call generated MCP tools named `figma_<operation_id_in_snake_case>` or CLI `fighorse figma api <operationId> --params '{...}'`. Readonly tools are available by default; Figma write tools require `FIGHORSE_MCP_MODE=write` or CLI `--yes`.

Figma file-browser links are navigation context, not design targets. First call `parse_figma_url` and follow `url_role`, `catalog_eligible`, and `next_action`. For `/files/.../team/<team-id>`, `/project/<project-id>`, or `/files/project/<project-id>`, call readonly `get_resource_catalog`; it enumerates accessible projects, files, branches, and team libraries and returns `ready`, `partial`, or `blocked` with structured permission guidance. Team/project enumeration requires `projects:read` and may require Projects endpoint approval; libraries need `team_library_content:read`, and optional depth-1 file probes need `file_content:read`. A `/files/<browser-root>` URL returns `browser_root_not_enumerable`, `catalog_eligible=false`, and cannot discover teams through the public REST API. After catalog selection, use a concrete file/selection URL with `get_design_package`. Never record real browser IDs, team/project IDs, file keys, names, URLs, or private catalog content in reusable experience.

## Code Connect

For modern Code Connect template workflows, use fighorse directly: CLI `fighorse code-connect generate|parse|validate|preview|publish|unpublish` or MCP `parse_code_connect_template`, `validate_code_connect`, `preview_code_connect`, `publish_code_connect`, and `unpublish_code_connect`. AI clients should read the target code repository with their own file tools and pass explicit component context; fighorse MCP does not scan or execute project code. MCP readonly mode exposes parse, validate, and preview tools; publish/unpublish appear only when `FIGHORSE_MCP_MODE=write`. MCP preview/publish sends template code to Figma and requires `FIGHORSE_MCP_CODE_CONNECT=allow`; publish/unpublish also requires `FIGHORSE_MCP_MODE=write`. Automatic Code Connect mapping discovery remains a Figma product capability; use the official Figma Remote MCP for that.

## Canvas Writes

Use `canvas_status`, `canvas_create_pairing`, `canvas_list_sessions`, `canvas_inspect`, `canvas_apply`, `canvas_capture`, `canvas_verify`, and `canvas_undo` for paired local Figma Design, FigJam, and Slides files. Write tools require both `FIGHORSE_MCP_MODE=write` and `FIGHORSE_CANVAS_MODE=write`, plus `yes=true`. If more than one session is connected, pass `session_id`; never guess a file. If a result is `unknown`, inspect or verify before doing anything else and do not resend the same plan automatically.

`canvas_execute_script` is a guarded escape hatch for Plugin API JavaScript. It appears only when `FIGHORSE_CANVAS_SCRIPT=allow` is also set and still requires `yes=true` on every call. Prefer structured operations.

## MCP Process Model

Installed clients should reuse the shared local MCP service at `http://127.0.0.1:9449/mcp` instead of spawning separate long-lived stdio servers. Use stdio only as an explicit compatibility mode for clients that cannot connect to the local HTTP endpoint.

The HTTP endpoint is the official Rust `rmcp` 2.2 Streamable HTTP service. It keeps independent stateful sessions, validates Host and Origin, returns a standard JSON or event-stream response according to content negotiation, and shuts down gracefully. Legacy `/sse` and `/messages` endpoints are not served; `--transport sse` fails with guidance to use `--transport http`.

Install the shared service for all primary clients with `fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply`, or install Claude alone with `fighorse install client --client claude --apply`. Service installation activates the service, waits for `/health`, completes `initialize` and `tools/list`, then writes client configurations. Finish with `fighorse install verify`; use `fighorse install rollback` to restore unchanged managed files from manifest backups.

Canonical AI instruction targets are:

- Cursor, Kimi, and Codex: `~/.agents/skills/fighorse/SKILL.md`
- Claude: `~/.claude/skills/fighorse/SKILL.md`
- Cursor rule: `~/.cursor/rules/fighorse.mdc`

## Assets

Use `export_images`, `export_component`, or `download_image_fills` with `manifest=true` for local slices, controls, icons, and image fills. MCP export requires `FIGHORSE_MCP_LOCAL_WRITE=allow` and still only writes inside approved export roots. Use `./.fighorse/exports` for temporary slices, `./assets/fighorse` or the app resource directory for packaged assets, and `~/.fighorse/exports` for cross-project scratch data. Do not write exports to protected system paths, dependency caches, or hard-to-discover temporary locations unless explicitly requested.

## Feedback Loop

Build/run the target app, capture screenshots, compare with Figma, fix overlap/clipping/status-bar/typography issues, then call `visual_audit` or `fighorse visual audit` for structured mismatch guidance. After a reusable fix, call `record_experience` or `fighorse experience add`.

## Complementary: Official Figma MCP

fighorse focuses on design-to-code read workflows, paired local canvas writes, and native modern Code Connect template workflows. Official Remote MCP: `https://mcp.figma.com/mcp` (OAuth, HTTP transport). Use official MCP for hosted Code to Canvas, Code Connect auto-mapping, Make resources, and product-only workflows. Use fighorse for design replication, local plugin canvas writes, native modern Code Connect template publish, asset export with manifests, visual audit, local experience learning, and transparent REST coverage. Both can be installed side-by-side in the same MCP client.

## Boundaries

fighorse is open-source and REST-transparent for REST features, and uses a local Figma plugin bridge for native canvas mutation. If the user asks for hosted product features such as Code to Canvas, automatic Code Connect mapping, Make resources, or official Remote MCP-only workflows, recommend installing the official Figma Remote MCP (`https://mcp.figma.com/mcp`) and offer the closest fighorse workflow as a fallback.
