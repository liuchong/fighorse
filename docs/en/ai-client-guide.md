# fighorse AI Client Guide

This guide is for AI coding tools and agents that use fighorse through MCP or CLI. The contract is simple: discover first, ask when product assumptions are missing, export assets with manifests, run a visual feedback loop, and record reusable lessons.

## Browser-Link Routing

When the user supplies a team or project browser URL, call readonly
`get_resource_catalog` before asking them to open every file. Preserve its
`ready`/`partial`/`blocked` diagnostics and let the user select a concrete file
before calling `get_design_package`. Do not treat `/files/<browser-root>` as a
team ID, and do not record catalog IDs, names, keys, URLs, or private content
as reusable experience. Always read `parse_figma_url` fields first:
`catalog_eligible=true` permits catalog calls, while
`browser_root_not_enumerable` means ask for a team/project URL or a concrete
design URL.

## Client Setup

Use the installer when possible:

```bash
fighorse install client --client cursor --apply
fighorse install client --client codex --apply
fighorse install client --client kimi --apply
fighorse install client --client claude --apply
fighorse install skill --clients cursor,codex,kimi,claude --apply
```

For generated configs without applying:

```bash
fighorse mcp config --client cursor --transport http
fighorse mcp config --client codex --transport http
fighorse mcp config --client kimi --transport http
fighorse mcp config --client claude --transport http
fighorse mcp config --client opencode --transport http
```

Recommended installed MCP config:

```json
{
  "mcpServers": {
    "fighorse": {
      "transport": "http",
      "url": "http://127.0.0.1:9449/mcp"
    },
    "figma-official": {
      "transport": "http",
      "url": "https://mcp.figma.com/mcp"
    }
  }
}
```

**Why both?** fighorse handles design-to-code read workflows (design package, asset export, visual audit, experience learning), local paired plugin canvas writes, and native modern Code Connect template workflows. The official Figma Remote MCP handles hosted product-only capabilities such as Code to Canvas, Code Connect auto-mapping, and Make resources. They complement each other and can coexist in the same client.

- Official Remote MCP: `https://mcp.figma.com/mcp` — OAuth auth, free during beta, will become usage-based paid.
- Seat requirements: Full seat for writes to shared files; Dev seat is read-only outside drafts.

Recommended local service:

```bash
fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply
fighorse install verify
```

Connect to `http://127.0.0.1:9449/mcp` with the official Rust `rmcp` 2.2 Streamable HTTP service. It keeps independent stateful sessions, validates Host and Origin, negotiates JSON or event-stream responses, and shuts down gracefully. Legacy `/sse` and `/messages` endpoints are absent; `--transport sse` fails with guidance to use HTTP. A `text/event-stream` response from `/mcp` is standard Streamable HTTP behavior, not the legacy transport. Standard stdio remains an explicit compatibility mode.

The installer activates the service, waits for `/health`, completes `initialize` and `tools/list`, then writes clients and skills. Managed hashes, backups, and `desired_absent` removals live in `~/.fighorse/install/manifest.json` and `~/.fighorse/install/backups/`; `fighorse install rollback` restores unchanged managed files and prior service state.

Canonical instruction targets are `~/.agents/skills/fighorse/SKILL.md` for Cursor/Kimi/Codex, `~/.claude/skills/fighorse/SKILL.md` for Claude, and `~/.cursor/rules/fighorse.mdc` for Cursor.

## Mandatory Startup Flow

When connected to fighorse, do this before implementation:

1. Call `discover_fighorse`.
2. Call `doctor` or read `discover_fighorse.production_defaults`.
3. Parse the user-provided Figma URL with `parse_figma_url` if needed.
4. If target platform or asset format is missing, ask the developer.
5. Call `list_experiences` for relevant platform, asset format, file key, and node type.
6. Call `get_design_package` with the Figma URL, platform, and asset format.
7. Export required images/components/fills with `manifest: true` if local assets are needed.
8. Implement the UI in the target codebase.
9. Run the app, capture screenshots, compare against Figma references, and call `visual_audit` for structured mismatch guidance.
10. Call `record_experience` for reusable lessons discovered during debugging.

Do not skip self-discovery. The manifest is part of the API contract and may evolve faster than hand-written client instructions.

## Ask Before Guessing

Ask the developer when any of these are missing:

- Target platform: web, Android Compose, iOS SwiftUI/UIKit, React Native, Flutter, desktop, etc.
- Asset format: png, svg, pdf, jpg, webp, or platform-specific vector format.
- Scope: exact screen/frame vs a broad CANVAS/user flow node.
- Destination for production assets when `./assets/fighorse` is not appropriate.

PNG is only the safest Figma node-render fallback. It is not a default product decision.

## Recommended MCP Tools

Use these high-level tools first:

- `discover_fighorse`: capabilities, contracts, safety defaults, recommended workflow.
- `doctor`: runtime/auth/local-write status.
- `parse_figma_url`: normalize file key and node id.
- `get_design_package`: structured implementation package.
- `list_experiences`: reusable local lessons.
- `record_experience`: write back reusable lessons.
- `visual_audit`: structure screenshot comparison, mismatch analysis, and experience suggestions.
- `get_project_playbook`: assemble project-level implementation rules from guidance and local experience.

Use these for assets:

- `export_images`: render node screenshots/slices.
- `export_component`: export component/control nodes as png/svg/pdf/jpg.
- `download_image_fills`: download image fills referenced by the design.

Use lower-level Figma tools only when the design package is insufficient:

- `get_file_compact`
- `get_node`
- `get_file_tree`
- `get_image`
- `get_image_fills`
- `get_file_tokens`

Use generated official REST tools when exact OpenAPI parity is needed. These tools are named `figma_<operation_id_in_snake_case>`, for example `figma_get_file`, `figma_get_developer_logs`, `figma_put_webhook`, and `figma_post_variables`. In readonly MCP mode, generated Figma write tools are hidden and blocked; set `FIGHORSE_MCP_MODE=write` only when the developer explicitly allows Figma mutations.

Use Code Connect tools only when the user is connecting code components to Figma Dev Mode:

- `parse_code_connect_template`: inspect already supplied Code Connect documents.
- `validate_code_connect`: check that target Figma nodes are components or component sets.
- `preview_code_connect`: send template code to Figma for real snippet rendering; requires `FIGHORSE_MCP_CODE_CONNECT=allow`.
- `publish_code_connect`: publish mappings; requires `FIGHORSE_MCP_CODE_CONNECT=allow` and `FIGHORSE_MCP_MODE=write`.
- `unpublish_code_connect`: delete precise node+label mappings; requires the same two switches.

For template generation, read the target codebase with the AI client's own file tools, then pass explicit component context to CLI `fighorse code-connect generate`. fighorse does not scan or execute the user's code repository through MCP.

Clients that support MCP resources and prompts can also read:

- `fighorse://capabilities`
- `fighorse://coverage`
- `fighorse://workflow/design-replication`
- `fighorse://experience/summary`
- Prompt: `fighorse_design_replication`
- Prompt: `fighorse_api_coverage`

## CLI Equivalents

If MCP is unavailable, run the equivalent CLI commands:

```bash
fighorse discover --format json
fighorse doctor --format json
fighorse url parse "<figma-url>"
fighorse experience summary --platform <platform> --asset-format <asset-format> --format json
fighorse design package "<figma-url>" --platform <platform> --asset-format <asset-format> --output ./.fighorse/exports/package.json
fighorse image export <file_key> --ids <node_ids> --dir ./.fighorse/exports --manifest
fighorse component export <file_key> --ids <node_ids> --format <asset-format> --dir ./assets/fighorse --manifest
fighorse asset download <file_key> --dir ./assets/fighorse --manifest
fighorse visual audit "<figma-url>" --screenshot <app-screenshot-path> --platform <platform> --asset-format <asset-format>
fighorse project playbook --platform <platform> --asset-format <asset-format>
fighorse figma-api coverage --format json
fighorse figma api getFile --params '{"file_key":"<file_key>","depth":1}'
fighorse code-connect generate "<figma-component-url>" --context code-context.json
fighorse code-connect parse --dir .
fighorse code-connect preview --documents docs.json
fighorse code-connect publish --documents docs.json --dry-run
```

## Local Write Policy

MCP Figma write mode and local filesystem write mode are independent.

- `FIGHORSE_MCP_MODE=readonly`: default; no Figma write tools exposed.
- `FIGHORSE_MCP_MODE=write`: exposes Figma write tools where implemented.
- `FIGHORSE_MCP_LOCAL_WRITE=deny`: default; local export tools are blocked.
- `FIGHORSE_MCP_LOCAL_WRITE=allow`: permits local export tools within approved roots.
- `FIGHORSE_MCP_CODE_CONNECT=deny`: default; Code Connect template code cannot be sent to Figma through MCP.
- `FIGHORSE_MCP_CODE_CONNECT=allow`: permits Code Connect preview/publish template egress to Figma.

Approved roots:

- `./.fighorse/exports`
- `./assets/fighorse`
- `~/.fighorse/exports`

Always request `manifest: true` for export tools. Read the manifest to locate files rather than inferring filenames.

## Design Package Contract

Treat `get_design_package` as the implementation source of truth. Important fields:

- `implementation_target`: platform and asset-format assumptions plus warnings.
- `target`: selected node identity, type, dimensions, and whether it is likely too broad.
- `scope`: `ready_to_implement` or `needs_narrowing`.
- `screen_candidates` and `component_candidates`: likely frames/components to inspect, narrow to, or export. Use `implementable=true` candidates for narrowing.
- `context`: compact design data for implementation.
- `tokens`: extracted colors, typography, spacing, and effects.
- `token_confidence` and `missing_font_diagnostics`: quality signals for token/font reliability.
- `screenshots`: render references returned by Figma.
- `asset_export_plan`: exact next export commands and MCP calls.
- `learned_experience`: lessons from previous runs.
- `implementation_risk_checklist`: concrete risks to verify before finalizing.
- `diagnostics`: readiness status, warnings, and screenshot `null_count`.

If `scope.status=needs_narrowing` for a `SECTION`, `CANVAS`, `DOCUMENT`, or
`SELECTION`, call `get_design_package` again with a `screen_candidates` node
where `implementable=true`. If `diagnostics.status` is not `ready`, follow the
warnings before coding when possible.

## Visual Fidelity Loop

Implementation is not complete after first code generation. Use this loop:

1. Export reference screenshots or slices from fighorse.
2. Implement the target screen using exact dimensions, typography, spacing, and colors from the package.
3. Run the app in the target platform.
4. Capture a screenshot at the intended viewport/device size.
5. Compare the app screenshot with the Figma reference.
6. Fix layout, typography, asset, clipping, scroll, status-bar, and overlap issues.
7. Repeat until differences are understood and acceptable.

Known lessons from real usage:

- Repeated siblings should map to platform list/linear containers, not generic stacking containers.
- Compact cards often need their own font size and line height; do not reuse full-card typography blindly.
- Mobile screens need scroll-safe layout decisions instead of fixed vertical stacks.
- Real device system UI can overlap a Figma status bar unless fullscreen or safe-area handling is explicit.
- Missing fonts should be diagnosed and handled intentionally.

## Recording Experiences

Record lessons that will help future, unrelated tasks. Good experiences are platform-aware but not project-specific:

```json
{
  "category": "layout",
  "platform": "android-compose",
  "asset_format": "png",
  "summary": "Repeated list items overlapped",
  "lesson": "Use a LazyColumn or Column for repeated sibling rows; use Box only for intentional overlays.",
  "tags": ["list", "overlap", "compose"]
}
```

Do not record:

- Secrets, tokens, private URLs, or local absolute paths.
- One-off project decisions that are not reusable.
- Lessons that merely restate the design content.

## Client-Specific Setup

All clients should receive the same fighorse contract. Differences should be limited to config file shape and transport. The recommended public setup is one shared local HTTP MCP service at `http://127.0.0.1:9449/mcp`.

### Cursor

Install:

```bash
fighorse install --default --mode service --clients cursor --apply
```

Expected config shape:

```json
{
  "mcpServers": {
    "fighorse": {
      "url": "http://127.0.0.1:9449/mcp"
    }
  }
}
```

Verify:

```bash
fighorse quickstart --format json
fighorse doctor --format json
```

Common failure: Cursor is configured to spawn stdio repeatedly. Replace that config with the shared HTTP endpoint unless the client cannot connect to localhost HTTP.

### Codex

Install:

```bash
fighorse install --default --mode service --clients codex --apply
```

Expected generated TOML:

```toml
[mcp_servers.fighorse]
url = "http://127.0.0.1:9449/mcp"
enabled = true
startup_timeout_sec = 60
```

Verify:

```bash
fighorse install status
curl http://127.0.0.1:9449/health
```

Common failure: Codex may initialize a new Streamable HTTP session each time it starts. `/mcp` must create an independent stateful session and return a standard MCP JSON or event-stream response. Restart the fighorse service after upgrading.

### Kimi

Install:

```bash
fighorse install --default --mode service --clients kimi --apply
```

Expected command shape:

```bash
kimi mcp add --transport http fighorse http://127.0.0.1:9449/mcp
```

Expected config payload: `{"transport":"http","url":"http://127.0.0.1:9449/mcp"}`.

Verify:

```bash
fighorse quickstart --format json
```

Common failure: older Kimi clients may only support stdio. Use `fighorse mcp config --client kimi --transport stdio` only for that compatibility case.

### Claude

Generate or install:

```bash
fighorse install client --client claude --apply
fighorse mcp config --client claude --transport http
```

Expected config shape:

```json
{
  "mcpServers": {
    "fighorse": {
      "type": "http",
      "url": "http://127.0.0.1:9449/mcp"
    }
  }
}
```

Verify by asking Claude to call `discover_fighorse`, then `check_fighorse_ready`.

Common failure: the desktop/client environment may not inherit shell tokens. Store the token with `fighorse auth login --token <FIGMA_TOKEN>` so the service can read local config.

### opencode

Install or generate:

```bash
fighorse install client --client opencode --apply
fighorse mcp config --client opencode --transport http
```

Expected config shape is the same HTTP MCP entry:

```json
{
  "mcpServers": {
    "fighorse": {
      "transport": "http",
      "url": "http://127.0.0.1:9449/mcp"
    }
  }
}
```

Verify:

```bash
fighorse doctor --format json
```

Common failure: service mode was not installed because `install all` defaults to CLI-only. Re-run with `--mode service`.

### VS Code-Compatible Clients

Use the generic HTTP MCP config unless the client documents a different schema:

```bash
fighorse mcp config --client generic --transport http
```

Expected shape:

```json
{
  "fighorse": {
    "transport": "http",
    "url": "http://127.0.0.1:9449/mcp"
  }
}
```

Common failure: the client expects an `mcpServers` wrapper. If so, use the Cursor-style shape above.

### Generic MCP

For Streamable HTTP:

```json
{
  "transport": "http",
  "url": "http://127.0.0.1:9449/mcp"
}
```

For explicit stdio compatibility only:

```json
{
  "command": "fighorse",
  "args": ["mcp", "serve", "--transport", "stdio"],
  "env": {
    "FIGHORSE_MCP_MODE": "readonly",
    "FIGHORSE_MCP_LOCAL_WRITE": "deny"
  }
}
```

Common failure: multiple long-lived stdio processes consume resources. Prefer the shared HTTP service for clients that support it.

When an AI tool sees a Figma URL and fighorse is available, it should not manually scrape the URL, guess frame ids, or implement from visual memory. Use fighorse first.

For native canvas writes, use the local plugin bridge rather than a REST token:
`canvas_status`, `canvas_create_pairing`, `canvas_list_sessions`,
`canvas_apply`, `canvas_verify`, and `canvas_undo`. The user must install and
run the Figma plugin. Write calls require `FIGHORSE_MCP_MODE=write`,
`FIGHORSE_CANVAS_MODE=write`, and `yes=true`; scripts additionally require
`FIGHORSE_CANVAS_SCRIPT=allow`. If multiple sessions are connected, ask or pass
the exact `session_id`. If the result is `unknown`, inspect or verify before any
next action.
