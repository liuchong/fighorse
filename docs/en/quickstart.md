# fighorse Quickstart

This guide gets a new user from zero to useful Figma context as quickly as possible. Start with CLI mode. Add MCP service mode only when you want an AI client to call fighorse directly.

## 1. Install

Build from source:

```bash
cargo build --release
./target/release/fighorse install --default --apply --source ./target/release/fighorse
```

`install --default --apply` copies the binary into the fighorse home, generates local config, and installs fighorse skills/instructions — the same self-install path used by a packaged binary.

Install a downloaded binary:

```bash
./fighorse install --default --apply
```

Install to a custom directory:

```bash
./fighorse install --path ~/.local/bin --apply
```

These default to CLI-only setup. They install the binary and local fighorse home, but do not start an MCP service or bind a port.

## 2. Add A Figma Token

Create a Figma personal access token with read access to file content. Then store it locally:

```bash
fighorse auth login --token <FIGMA_TOKEN>
```

You can also keep the token out of config and run one-off commands with `FIGMA_TOKEN=<token>`.

## 3. Verify Setup

Run the guided check:

```bash
fighorse quickstart
```

For machine-readable output:

```bash
fighorse quickstart --format json
```

## 4. Copy A Specific Figma Link

In Figma, select the exact frame, component, or group you want to implement. Copy a link to that selection. Avoid starting from a whole page or broad canvas unless you are exploring.

Verify the link:

```bash
fighorse quickstart "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
```

## 5. Get The Design Package

Ask the target platform and asset format first. Then build the package:

```bash
fighorse design package "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>" \
  --platform web-react \
  --asset-format svg \
  --output ./.fighorse/exports/package.json
```

This is the main context source for AI implementation. It includes compact structure, screenshots, tokens, diagnostics, asset export suggestions, and local learned experience.

## 6. Optional: Use MCP Service Mode

Only use service mode when an AI client should call fighorse directly:

```bash
fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply
fighorse install verify
```

For Claude only:

```bash
fighorse install client --client claude --apply
```

Installed clients should use:

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

The service is localhost-only by default, guarded by a singleton lock, and uses the official Rust `rmcp` 2.2 Streamable HTTP service. Each client gets an independent stateful session. Host and Origin are validated before dispatch, responses are standard JSON or event-stream responses, and SIGINT/SIGTERM performs graceful shutdown.

The native HTTP entry is `{"url":"http://127.0.0.1:9449/mcp"}` for Cursor, `{"transport":"http","url":"http://127.0.0.1:9449/mcp"}` for Kimi, `{"type":"http","url":"http://127.0.0.1:9449/mcp"}` for Claude, and `[mcp_servers.fighorse]` with the same URL for Codex.

Installation applies service files first, waits for `/health`, completes `initialize` and `tools/list`, and only then writes client configs and skills. `~/.fighorse/install/manifest.json` records managed files and `desired_absent` removals; backups are under `~/.fighorse/install/backups/`. Use `fighorse install rollback` only while managed files still match the manifest.

Canonical instruction targets are `~/.agents/skills/fighorse/SKILL.md` for Cursor/Kimi/Codex, `~/.claude/skills/fighorse/SKILL.md` for Claude, and `~/.cursor/rules/fighorse.mdc` for Cursor.

Legacy `/sse` and `/messages` endpoints are not served. `--transport sse` fails and directs the user to `--transport http`; a `text/event-stream` response from `/mcp` is standard Streamable HTTP response negotiation, not the retired legacy transport. Fresh service and stdio configs deny local writes.

## 7. Optional: Code Connect

For modern Code Connect templates:

```bash
fighorse code-connect generate "<figma-component-url>" --context code-context.json
fighorse code-connect parse --dir .
fighorse code-connect publish --documents docs.json --dry-run
```

Use `--yes` only after reviewing the dry-run output. MCP preview/publish also requires `FIGHORSE_MCP_CODE_CONNECT=allow`; publish/unpublish requires `FIGHORSE_MCP_MODE=write`.

## 8. What To Ask Your AI Agent

After MCP is connected, paste a specific Figma frame link and ask:

```text
Use fighorse to inspect this Figma frame. First call discover_fighorse, then list_experiences, then get_design_package. Ask me for platform or asset format if missing. Export assets with manifests instead of guessing.
```

## Troubleshooting

- Token missing: run `fighorse auth login --token <FIGMA_TOKEN>`.
- Link is too broad: copy a link to a selected frame or component.
- MCP service not running: use `fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply`.
- Codex reports unexpected content type: run `fighorse install verify`; `/mcp` must return a standard MCP JSON or event-stream response, not a product manifest.
- Local export rejected: use `./.fighorse/exports`, `./assets/fighorse`, or `~/.fighorse/exports`.
