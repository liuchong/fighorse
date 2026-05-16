# fighorse AI Client Guide

This guide is for AI coding tools and agents that use fighorse through MCP or CLI. The contract is simple: discover first, ask when product assumptions are missing, export assets with manifests, run a visual feedback loop, and record reusable lessons.

## Client Setup

Use the installer when possible:

```bash
fighorse install client --client cursor --apply
fighorse install client --client codex --apply
fighorse install client --client kimi --apply
fighorse install skill --clients cursor,codex,kimi --apply
```

For generated configs without applying:

```bash
fighorse mcp config --client cursor --transport stdio
fighorse mcp config --client codex --transport stdio
fighorse mcp config --client claude --transport sse
fighorse mcp config --client opencode --transport stdio
```

Recommended stdio MCP config:

```json
{
  "mcpServers": {
    "fighorse": {
      "command": "fighorse",
      "args": ["mcp", "serve", "--transport", "stdio"],
      "env": {
        "FIGMA_MCP_MODE": "readonly",
        "FIGHORSE_MCP_MODE": "readonly",
        "FIGHORSE_MCP_LOCAL_WRITE": "allow"
      }
    }
  }
}
```

Recommended SSE service:

```bash
FIGHORSE_MCP_MODE=readonly FIGHORSE_MCP_LOCAL_WRITE=allow \
  fighorse mcp serve --transport sse --host 127.0.0.1 --port 9449
```

Connect to `http://127.0.0.1:9449/sse`.

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
```

## Local Write Policy

MCP Figma write mode and local filesystem write mode are independent.

- `FIGHORSE_MCP_MODE=readonly`: default; no Figma write tools exposed.
- `FIGHORSE_MCP_MODE=write`: exposes Figma write tools where implemented.
- `FIGHORSE_MCP_LOCAL_WRITE=deny`: default; local export tools are blocked.
- `FIGHORSE_MCP_LOCAL_WRITE=allow`: permits local export tools within approved roots.

Approved roots:

- `./.fighorse/exports`
- `./assets/fighorse`
- `~/.fighorse/exports`

Always request `manifest: true` for export tools. Read the manifest to locate files rather than inferring filenames.

## Design Package Contract

Treat `get_design_package` as the implementation source of truth. Important fields:

- `implementation_target`: platform and asset-format assumptions plus warnings.
- `target`: selected node identity, type, dimensions, and whether it is likely too broad.
- `screen_candidates` and `component_candidates`: likely frames/components to inspect, narrow to, or export.
- `context`: compact design data for implementation.
- `tokens`: extracted colors, typography, spacing, and effects.
- `token_confidence` and `missing_font_diagnostics`: quality signals for token/font reliability.
- `screenshots`: render references returned by Figma.
- `asset_export_plan`: exact next export commands and MCP calls.
- `learned_experience`: lessons from previous runs.
- `implementation_risk_checklist`: concrete risks to verify before finalizing.
- `diagnostics`: readiness status and warnings.

If `diagnostics.status` is not `ready`, follow the warnings before coding when possible. A CANVAS or user-flow target should usually be narrowed to a concrete mobile/web frame.

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

## Cursor, Codex, Kimi, Claude, opencode Notes

All clients should receive the same fighorse contract. Differences should be limited to config file shape and transport.

- Cursor: use `install client --client cursor --apply` where possible; it also installs rules/skills for agent behavior.
- Codex: use `install client --client codex --apply`; generated TOML includes local-write environment when configured.
- Kimi: use `install client --client kimi --apply`; generated config follows Kimi MCP conventions.
- Claude and opencode: generate config with `mcp config` or install client if supported on the host.

When an AI tool sees a Figma URL and fighorse is available, it should not manually scrape the URL, guess frame ids, or implement from visual memory. Use fighorse first.
