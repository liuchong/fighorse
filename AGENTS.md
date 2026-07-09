# AI Agent Guide

This repository is a Rust CLI and MCP server for turning Figma REST API data into AI-friendly context.

## Hard Rules

- Use Cargo for all build, test, and binary workflows.
- Never commit Figma access tokens, exported private design data, local config files, or generated binaries.
- Preserve user changes in the working tree. Do not reset, checkout, or remove unrelated edits.
- MCP Figma write mode and local file write mode are separate. Local asset export requires `FIGHORSE_MCP_LOCAL_WRITE=allow` and must stay under approved export roots.
- Installed MCP clients should reuse the local HTTP service at `http://127.0.0.1:9449/mcp`; do not default to spawning multiple long-lived stdio servers. The MCP server uses a singleton lock unless `FIGHORSE_MCP_ALLOW_MULTIPLE=1` is explicitly set for development.

## Commands

```bash
cargo build --release
cargo test
cargo clippy
```

Use integration tests only when a real token is intentionally provided:

```bash
FIGMA_INTEGRATION_TESTS=1 FIGMA_TOKEN=<token> cargo test -- --ignored
```

Run the compiled CLI:

```bash
./target/release/fighorse --help
./target/release/fighorse file tree <file_key>
./target/release/fighorse file compact <file_key> --depth 2 --max-tokens 8000
```

Run the MCP server:

```bash
./target/release/fighorse mcp serve --transport sse --port 9449
./target/release/fighorse mcp serve --transport stdio  # explicit compatibility mode only
```

## Project Shape

- `src/main.rs` contains CLI routing; `src/cli/` holds command IO.
- `src/api/*` wraps Figma REST API endpoints (`coverage.rs`, `operations.rs`, `mod.rs`).
- `src/transform/*` (`compact.rs`, `tokens.rs`, `filter.rs`, `schema.rs`, `diff.rs`) turn raw Figma JSON into AI-useful artifacts.
- `src/mcp/*` exposes the same capabilities to MCP clients.
- `src/product/*`, `src/discovery.rs`, `src/experience.rs`, `src/install.rs` provide the design package, self-discovery, learning, and installer.
- Unit tests live inline (`#[cfg(test)]`); integration tests are under `tests/`. Real Figma API tests are opt-in through `FIGMA_INTEGRATION_TESTS=1`.

## Development Expectations

- When adding a Figma capability, expose it through the API layer, CLI where useful, MCP tools where useful, and tests.
- Prefer small, deterministic unit tests with mocked HTTP (`wiremock`). Keep real API tests behind the integration flag.
- Keep generated output out of git unless it is a deliberate fixture.
- Use `FIGMA_TOKEN` for one-off authenticated commands, or `fighorse auth login --token <token>` for local-only config.
- Keep CLI stdout machine-readable for data commands. Send diagnostics to stderr.
- Long-running services must be idle-efficient, release locks on SIGINT/SIGTERM/stdin close, and avoid leaving orphaned MCP processes.
- One-shot CLI commands must not use the MCP singleton lock; they should bound network work with timeouts and exit cleanly after output is written.
- `install all` defaults to CLI-only setup. Long-running MCP service setup must be explicit with `install all --mode service` or `install service`; do not configure, kickstart, or bind the MCP service in CLI-only workflows.
- Streamable HTTP `/mcp` must support repeated client handshakes: the handler dispatches one stateless JSON-RPC message per request, so Codex-style repeated initialize handshakes stay valid.
- Keep README, discovery output, generated skills, MCP schemas, and actual CLI behavior consistent when changing workflows or defaults.

## AI Tool Notes

- Cursor should read `.cursor/rules/fighorse.mdc`, which points back to this file.
- Codex, Kimi CLI, opencode, and similar agents should treat this file as the repository operating contract.
- If tool-specific memory is needed later, duplicate only the short Cargo-only command policy and link back here.
