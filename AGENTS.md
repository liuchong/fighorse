# AI Agent Guide

This repository is a Bun-first ClojureScript CLI and MCP server for turning Figma REST API data into AI-friendly context.

## Hard Rules

- Use Bun for all JavaScript package, script, test, and binary workflows.
- Do not run alternate JavaScript package managers or runtimes directly in this repository.
- Shadow CLJS target names such as `:node-script` and `:node-test` are compiler target names only; execute their output with Bun.
- Never commit Figma access tokens, exported private design data, local config files, or generated binaries.
- Preserve user changes in the working tree. Do not reset, checkout, or remove unrelated edits.
- MCP Figma write mode and local file write mode are separate. Local asset export requires `FIGHORSE_MCP_LOCAL_WRITE=allow` and must stay under approved export roots.
- Installed MCP clients should reuse the local HTTP service at `http://127.0.0.1:9449/mcp`; do not default to spawning multiple long-lived stdio servers. The MCP server uses a singleton lock unless `FIGHORSE_MCP_ALLOW_MULTIPLE=1` is explicitly set for development.

## Commands

```bash
bun install
bun run test
bun run build
bun run compile
bun run check
```

Use integration tests only when a real token is intentionally provided:

```bash
FIGMA_TOKEN=<token> bun run test:integration
```

Run the compiled CLI:

```bash
./dist/fighorse --help
./dist/fighorse file tree <file_key>
./dist/fighorse file compact <file_key> --depth 2 --max-tokens 8000
```

Run the MCP server:

```bash
./dist/fighorse mcp serve --transport sse --port 9449
./dist/fighorse mcp serve --transport stdio  # explicit compatibility mode only
```

## Project Shape

- `src/fighorse/core.cljs` contains CLI routing and command IO.
- `src/fighorse/api/*` wraps Figma REST API endpoints.
- `src/fighorse/compact.cljs`, `tokens.cljs`, `filter.cljs`, `schema.cljs`, and `diff.cljs` turn raw Figma JSON into AI-useful artifacts.
- `src/fighorse/mcp/*` exposes the same capabilities to MCP clients.
- `test/fighorse/*` contains local tests. Real Figma API tests are opt-in through `FIGMA_INTEGRATION_TESTS=true`.

## Development Expectations

- When adding a Figma capability, expose it through the API layer, CLI where useful, MCP tools where useful, and tests.
- Prefer small, deterministic unit tests with mocked `fetch`. Keep real API tests behind the integration flag.
- Keep generated output out of git unless it is a deliberate fixture.
- Use `FIGMA_TOKEN` for one-off authenticated commands, or `fighorse auth login --token <token>` for local-only config.
- Keep CLI stdout machine-readable for data commands. Send diagnostics to stderr.
- Long-running services must be idle-efficient, release locks on SIGINT/SIGTERM/stdin close, and avoid leaving orphaned MCP processes.
- One-shot CLI commands must not use the MCP singleton lock; they should bound network work with timeouts, abort in-flight requests on signals, and exit cleanly after output is written.
- `install all` defaults to CLI-only setup. Long-running MCP service setup must be explicit with `install all --mode service` or `install service`; do not configure, kickstart, or bind the MCP service in CLI-only workflows.
- Keep README, discovery output, generated skills, MCP schemas, and actual CLI behavior consistent when changing workflows or defaults.

## AI Tool Notes

- Cursor should read `.cursor/rules/fighorse.mdc`, which points back to this file.
- Codex, Kimi CLI, opencode, and similar agents should treat this file as the repository operating contract.
- If tool-specific memory is needed later, duplicate only the short Bun-only command policy and link back here.
