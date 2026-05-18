# fighorse Quickstart

This guide gets a new user from zero to useful Figma context as quickly as possible. Start with CLI mode. Add MCP service mode only when you want an AI client to call fighorse directly.

## 1. Install

Build from source:

```bash
bun install
bun run build
bun run compile
./dist/fighorse install all --apply --source ./dist/fighorse
```

`install all` defaults to CLI-only setup. It installs the binary and local fighorse home, but it does not start an MCP service or bind a port.

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
fighorse install all --mode service --clients cursor,codex,kimi --apply --source ./dist/fighorse
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

The service is localhost-only by default, guarded by a singleton lock, and uses a Streamable HTTP endpoint that supports repeated client handshakes.

## 7. What To Ask Your AI Agent

After MCP is connected, paste a specific Figma frame link and ask:

```text
Use fighorse to inspect this Figma frame. First call discover_fighorse, then list_experiences, then get_design_package. Ask me for platform or asset format if missing. Export assets with manifests instead of guessing.
```

## Troubleshooting

- Token missing: run `fighorse auth login --token <FIGMA_TOKEN>`.
- Link is too broad: copy a link to a selected frame or component.
- MCP service not running: use `fighorse install all --mode service --apply --source ./dist/fighorse`.
- Codex reports unexpected content type: verify `curl http://127.0.0.1:9449/health`; the `/mcp` endpoint must return MCP JSON/SSE, not `text/plain`.
- Local export rejected: use `./.fighorse/exports`, `./assets/fighorse`, or `~/.fighorse/exports`.
