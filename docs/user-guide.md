# fighorse User Guide

`fighorse` is a public-first, open-source Figma CLI + MCP. It turns public Figma REST API data into AI-friendly context and developer-friendly CLI output. The first run should be simple: install, add a token, paste a specific frame link, and generate a useful design package. The second run can go deeper with asset manifests, visual audit, project playbooks, full REST coverage, and local experience learning.

Use the CLI when you want reproducible commands, scripts, CI, or quick inspection. Use the MCP service when an AI coding tool should call fighorse directly.

## Install From Source

For the fastest path, see [Quickstart](quickstart.md). The default install mode is CLI-only and does not start MCP service processes.

```bash
bun install
bun run build
bun run compile
./dist/fighorse --help
```

Install the compiled binary and optional AI client integrations:

```bash
./dist/fighorse install status
./dist/fighorse install auth --apply
./dist/fighorse install binary --source ./dist/fighorse --apply
./dist/fighorse install all --apply --source ./dist/fighorse
```

Install MCP service and AI client integrations only when needed:

```bash
./dist/fighorse install client --client cursor --apply
./dist/fighorse install client --client codex --apply
./dist/fighorse install client --client kimi --apply
./dist/fighorse install service --service launchd --apply
./dist/fighorse install all --mode service --clients cursor,codex,kimi --source ./dist/fighorse --apply
```

Install commands generate reviewable files by default. Add `--apply` only when you want fighorse to mutate user-level client config, skill/rule locations, binary links, or service managers.

## Authentication

Create a Figma personal access token from the Figma developer settings, then store it locally:

```bash
fighorse auth login --token <FIGMA_TOKEN>
```

You can also use environment variables for one-off commands:

```bash
FIGMA_TOKEN=<FIGMA_TOKEN> fighorse file tree <file_key>
```

`install auth --apply` reads `--token`, stdin, `FIGMA_TOKEN`, or `FIGMA_API_KEY` and stores the token in `~/.fighorse/config.json`. Command output masks the token.

## Verify Readiness

```bash
fighorse quickstart
fighorse quickstart "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
fighorse doctor --format json
fighorse discover --format json
fighorse smoke "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
fighorse figma-api coverage --format json
```

`smoke` uses real Figma access and returns `fighorse.smoke.v1`. `ok: true` means the normal design package path is ready. `ok: false` with `diagnostics.status: partial` can still mean access worked; follow the warnings, usually by specifying a platform, asset format, or exact frame node.

`figma-api coverage` reports parity against the vendored Figma REST OpenAPI snapshot. The current registry tracks 48 public operations and exposes each through the API layer, generic CLI operation dispatch, and generated official MCP tools where safe.

## Official REST API Coverage

Use the product commands for normal design work, and use the generic REST dispatch when you need exact low-level API parity:

```bash
fighorse figma-api coverage --format md
fighorse figma api getFile --params '{"file_key":"<file_key>","depth":1}'
fighorse figma api putWebhook --params '{"webhook_id":"<id>"}' --body '{"status":"PAUSED"}' --yes
```

`figma api` accepts official `operationId` names from the OpenAPI registry. Read operations run normally. Write operations require `--yes` because they can mutate comments, variables, webhooks, or dev resources.

## Inspect Designs

Parse a pasted Figma URL:

```bash
fighorse url parse "https://www.figma.com/design/<fileKey>/<name>?node-id=1-2"
```

View structure:

```bash
fighorse file tree <file_key> --depth 2
fighorse node get <file_key> <node_id> --depth 3
```

Fetch compact AI context:

```bash
fighorse file compact <file_key> --depth 2 --max-tokens 8000
fighorse file get <file_key> --depth 2 | fighorse compact --max-tokens 8000
```

Extract design tokens:

```bash
fighorse file tokens <file_key> --format json
fighorse tokens extract <file_key> --format css --output ./tokens.css
```

## Build A Design Package

For AI implementation work, prefer `design package` over low-level calls:

```bash
fighorse design package "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>" \
  --platform android-compose \
  --asset-format png \
  --max-tokens 8000 \
  --output ./.fighorse/exports/package.json
```

The package includes:

- `source`: parsed file key and node id.
- `file` and `target`: metadata and selected node summary.
- `implementation_target`: platform and asset-format assumptions.
- `screen_candidates` and `component_candidates`: likely frames/components to inspect or export next.
- `fidelity_workflow`: visual verification steps.
- `asset_export_plan`: suggested CLI and MCP asset export calls.
- `learned_experience`: local lessons from previous runs.
- `token_confidence`, `missing_font_diagnostics`, and `implementation_risk_checklist`: AI-ready checks before coding.
- `context`, `tokens`, `screenshots`, and optional `assets`.
- `diagnostics`: warnings for missing platform, missing asset format, CANVAS targets, truncation, missing screenshots, or missing tokens.

If platform or asset format is unknown, ask the developer before implementation. PNG is a render fallback, not an automatic product decision.

## Export Assets

Use local exports when implementation needs real image files instead of temporary render URLs:

```bash
fighorse image export <file_key> --ids <node_ids> --format png --dir ./.fighorse/exports --manifest
fighorse component export <file_key> --ids <component_node_ids> --format svg --dir ./assets/fighorse --manifest
fighorse asset download <file_key> --dir ./assets/fighorse --manifest
```

Recommended output locations:

- `./.fighorse/exports`: temporary slices, screenshots, manifests, and debug assets.
- `./assets/fighorse`: assets intended to be referenced by app code or packaged.
- `~/.fighorse/exports`: cross-project scratch exports.

Export commands write safe filenames and can create `manifest.json`. Use the manifest instead of parsing terminal output.

## MCP Server

For installed clients, prefer the shared local HTTP service so Cursor, Codex, Kimi, and other clients reuse one `fighorse` process instead of each spawning a stdio subprocess:

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

Install and start the local service through the explicit service path when possible:

```bash
fighorse install all --mode service --clients cursor,codex,kimi --apply --source ./dist/fighorse
```

For development, you can also run it directly with `fighorse mcp serve --transport sse --host 127.0.0.1 --port 9449`.

HTTP endpoints:

```text
http://127.0.0.1:9449/mcp
http://127.0.0.1:9449/sse
http://127.0.0.1:9449/manifest
http://127.0.0.1:9449/health
```

The service binds to `127.0.0.1` by default and uses a singleton lock in `~/.fighorse/runtime`. Use `--host` explicitly only when you intend to expose the service beyond localhost. Use stdio only for a client that cannot connect to the local HTTP endpoint.

Implementation note for maintainers: Streamable HTTP clients such as Codex may initialize a fresh MCP connection every time they start. The `/mcp` endpoint must therefore tolerate repeated handshakes. In stateless mode, create and close a fresh transport/server per request. Reusing one already-initialized `StreamableHTTPServerTransport` can make the first handshake pass and later handshakes fail with `500 text/plain` instead of an MCP JSON/SSE response.

Normal CLI commands such as `fighorse file get`, `fighorse design package`, and `fighorse image export` are one-shot processes. They are allowed to start each time, do not start the MCP service, do not bind ports, do not take the MCP singleton lock, and should exit after output is written. Figma HTTP calls and image downloads use `FIGHORSE_HTTP_TIMEOUT_MS` with a default of `120000`, and `SIGINT`/`SIGTERM` abort in-flight requests before exiting. `fighorse install all` defaults to CLI-only setup; use `fighorse install all --mode service --apply` or `fighorse install service --apply` only when you explicitly want fighorse to configure or kickstart a long-running MCP service.

## Safety Modes

Figma write tools are hidden unless enabled:

```bash
FIGHORSE_MCP_MODE=write fighorse mcp serve --transport sse
```

Local file export is controlled separately:

```bash
FIGHORSE_MCP_LOCAL_WRITE=allow fighorse mcp serve --transport sse
```

Even with local write enabled, export paths are validated and must stay under `./.fighorse/exports`, `./assets/fighorse`, or `~/.fighorse/exports`.

## Experience Store

fighorse stores reusable lessons as append-only JSONL. This lets future AI runs learn from previous visual debugging without changing the fighorse binary.

Default paths:

- Home: `~/.fighorse`.
- Global experience: `~/.fighorse/experience/global.jsonl`.
- Project experience: `./.fighorse/experience.jsonl` after `fighorse install project`.
- Exact override: `FIGHORSE_EXPERIENCE_PATH`.
- Scope override: `FIGHORSE_EXPERIENCE_SCOPE=global|project|merged`.

Commands:

```bash
fighorse install project
fighorse experience schema
fighorse experience summary --platform android-compose --asset-format png --format md
fighorse experience add \
  --summary "Repeated list items overlapped" \
  --lesson "Use a list or linear container for repeated siblings; use stacking containers only for intentional overlays." \
  --category layout \
  --platform android-compose \
  --asset-format png
```

## Common Workflows

Use fighorse before implementing a Figma screen:

```bash
fighorse discover --format json
fighorse experience summary --platform web-react --asset-format svg
fighorse design package "<figma-url>" --platform web-react --asset-format svg --output ./.fighorse/exports/package.json
fighorse visual audit "<figma-url>" --screenshot ./.fighorse/exports/app-screen.png --platform web-react --asset-format svg
fighorse project playbook --platform web-react --asset-format svg
```

Sync tokens:

```bash
fighorse file tokens <design_system_key> --format css --output src/styles/tokens.css
```

Batch export frames:

```bash
IDS=$(fighorse file get <file_key> --depth 2 | jq -r '.. | objects | select(.type == "FRAME") | .id' | paste -sd, -)
fighorse image export <file_key> --ids "$IDS" --dir ./.fighorse/exports --manifest
```

## Troubleshooting

- First run is unclear: run `fighorse quickstart "<figma-frame-url>"` and follow `next_steps`.
- `doctor.auth.has_token` is false: run `fighorse auth login --token <FIGMA_TOKEN>` or `fighorse install auth --apply`.
- `doctor.checks` reports MCP service not running: ignore it for CLI-only work, or run `fighorse install all --mode service --apply --source ./dist/fighorse` for AI clients.
- Codex/Cursor reports `text/plain` or repeated initialize failures: restart the fighorse service after upgrading; `/mcp` must support repeated Streamable HTTP handshakes.
- `smoke.ok` is false but file metadata exists: follow `diagnostics.warnings`; often the selected target is too broad or platform/asset format is missing.
- MCP export tool reports local-write disabled: set `FIGHORSE_MCP_LOCAL_WRITE=allow` in the MCP server environment.
- Export path is rejected: use `./.fighorse/exports`, `./assets/fighorse`, or `~/.fighorse/exports`.
- AI implements an entire user flow page: narrow the Figma URL to a concrete Frame/Screen node before implementation.
