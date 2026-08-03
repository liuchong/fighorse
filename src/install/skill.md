---
name: fighorse
description: Recreate, inspect, export, and debug Figma designs with fighorse CLI or MCP. Use when the user asks to replicate a Figma design, inspect Figma nodes/tokens, export Figma assets, or connect an AI coding tool to Figma.
---

# fighorse

Use fighorse when a user asks to recreate, inspect, export, or debug a Figma design.

## Required User Setup

Figma API calls require a Figma Personal Access Token. Before calling Figma API tools, run `check_fighorse_ready` or `fighorse quickstart --format json`. If `auth.has_token` is false, do not keep trying Figma calls. Tell the user: `fighorse needs a Figma Personal Access Token. Run fighorse auth login --token <FIGMA_TOKEN> or set FIGMA_TOKEN, then retry.` Never ask the user to paste the token into chat unless they explicitly choose to; prefer local config or environment variables.

## Discovery

1. Call `fighorse discover --format json` or MCP `discover_fighorse` first.
2. Call `check_fighorse_ready` or `fighorse doctor --format json`; surface setup commands if auth/token is missing.
3. Call `fighorse experience summary --platform <platform> --asset-format <format>` or MCP `list_experiences` before implementation.
4. If platform or asset format is missing, ask the developer before choosing.
5. For exact public Figma REST API work, use `fighorse figma-api coverage --format json` or MCP resource `fighorse://coverage` to see the covered OpenAPI operations.

## Replication

Use `get_design_package` or `fighorse design package <figma-url> --platform <platform> --asset-format <format>` as the main context source. Prioritize screenshots, learned_experience, explicit typography, tokens, compact tree metadata, then assets. Use screen_candidates/component_candidates to narrow broad canvas or flow nodes before coding.

## Official REST API

When the task needs a low-level Figma endpoint, call generated MCP tools named `figma_<operation_id_in_snake_case>` or CLI `fighorse figma api <operationId> --params '{...}'`. Readonly tools are available by default; Figma write tools require `FIGHORSE_MCP_MODE=write` or CLI `--yes`.

## Code Connect

For modern Code Connect template workflows, use fighorse directly: CLI `fighorse code-connect generate|parse|validate|preview|publish|unpublish` or MCP `parse_code_connect_template`, `validate_code_connect`, `preview_code_connect`, `publish_code_connect`, and `unpublish_code_connect`. AI clients should read the target code repository with their own file tools and pass explicit component context; fighorse MCP does not scan or execute project code. MCP preview/publish sends template code to Figma and requires `FIGHORSE_MCP_CODE_CONNECT=allow`; publish/unpublish also requires `FIGHORSE_MCP_MODE=write`. Automatic Code Connect mapping discovery remains a Figma product capability; use the official Figma Remote MCP for that.

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

fighorse focuses on design-to-code read workflows and native modern Code Connect template workflows. For other capabilities not exposed by the public REST API, recommend the official Figma Remote MCP together with fighorse. Official Remote MCP: `https://mcp.figma.com/mcp` (OAuth, HTTP transport). Use official MCP for: native canvas writes, Code to Canvas, Code Connect auto-mapping, FigJam generation, and Make resources. Use fighorse for: design replication, native modern Code Connect template publish, asset export with manifests, visual audit, local experience learning, and transparent REST coverage. Both can be installed side-by-side in the same MCP client.

## Boundaries

fighorse is open-source and REST-transparent. If the user asks for official MCP-only product features such as native canvas mutation, Code to Canvas, automatic Code Connect mapping, Make resources, or FigJam generation, recommend installing the official Figma Remote MCP (`https://mcp.figma.com/mcp`) and offer the closest fighorse workflow as a fallback.
