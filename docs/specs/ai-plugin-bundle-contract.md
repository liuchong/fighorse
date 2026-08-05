# AI Plugin Bundle Contract

fighorse ships a local-only AI plugin bundle so users can install one coherent
set of MCP configuration, agent rules, and workflow skills across common AI
clients without publishing to any marketplace.

## Scope

- The source templates live under `assets/ai-plugin/`; generated files live
  under `~/.fighorse/ai-plugin/fighorse/`.
- Generate or install with
  `fighorse install ai-plugin --clients cursor,codex,kimi,claude,opencode,gemini --apply`.
- The default MCP endpoint is `http://127.0.0.1:9449/mcp`.
- The bundle is local-only. It must not claim Cursor verification or public
  marketplace availability.
- Installed MCP clients must reuse the local HTTP service instead of spawning
  independent long-running stdio servers.
- The bundle must not contain Figma tokens, exported design data, pairing codes,
  node IDs, screenshots, or runtime session state.

## Shared Skills

The bundle exposes one entry skill plus workflow skills:

- `fighorse`
- `fighorse-design-to-code`
- `fighorse-canvas-write`
- `fighorse-resource-catalog`
- `fighorse-code-connect`
- `fighorse-self-learning`

All clients should consume the same skill source where possible. Client-specific
manifests are thin wrappers around shared skills and shared MCP configuration.

## MCP and Permissions

The MCP server configuration must stay readonly by default. Installing the AI
plugin bundle must not enable canvas writes, arbitrary JavaScript, Code Connect
publishing, or local file export. Those remain gated by the existing fighorse
environment variables and per-call confirmations.

## rmcp Upgrade Gate

fighorse already uses the official MCP Rust SDK crate `rmcp`. This task only
evaluates whether to move beyond `rmcp 2.2.0`. If upgrading requires handler,
transport, schema, or handshake rewrites, the upgrade is deferred to a separate
task.

## BDD Scenarios

1. Given default settings, when fighorse renders the AI plugin bundle, then the
   Cursor manifest points to shared skills and `.mcp.json`, and `.mcp.json`
   points to `http://127.0.0.1:9449/mcp`.
2. Given the full client list, when fighorse renders the bundle, then Cursor,
   Claude, Gemini, generic MCP, and shared skills are all present.
3. Given a dry-run install, when the user requests the AI plugin bundle, then
   no user directory is mutated and the returned file list is complete.
4. Given an apply install into a temporary home, when the transaction succeeds,
   then the package is written under `ai-plugin/fighorse`, shared skills are
   installed, and no secret-shaped values are present.
5. Given an unsupported or unverified automatic client path, when apply runs,
   then fighorse produces artifacts and manual actions instead of guessing a
   private configuration location.
6. Given the rmcp upgrade gate, when the task runs, then it records whether the
   current official SDK version is retained or upgraded with test evidence.

