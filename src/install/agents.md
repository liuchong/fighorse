# fighorse Agent Instructions

- Start with `fighorse discover --format json` or MCP `discover_fighorse`.
- Then call `check_fighorse_ready` or `fighorse doctor --format json`. If `auth.has_token` is false, tell the user to run `fighorse auth login --token <FIGMA_TOKEN>` or set `FIGMA_TOKEN`; do not retry Figma API calls until setup is fixed.
- Load local lessons with `list_experiences` before using `get_design_package`.
- Use `platform` and `asset_format` explicitly; ask if unknown.
- Prefer the installed shared HTTP MCP endpoint `http://127.0.0.1:9449/mcp`; avoid starting duplicate long-lived stdio servers unless a client requires compatibility mode.
- The shared endpoint uses official Rust `rmcp` 2.2 Streamable HTTP with independent stateful sessions, Host/Origin validation, JSON or event-stream response negotiation, and graceful shutdown. Legacy `/sse` and `/messages` endpoints are retired; `--transport sse` must fail and migrate to HTTP.
- Install Cursor, Codex, Kimi, and Claude with `fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply`; install Claude alone with `fighorse install client --client claude --apply`. Service activation and `/health` plus `initialize`/`tools/list` verification happen before client configuration.
- Canonical instruction targets are `~/.agents/skills/fighorse/SKILL.md` for Cursor/Kimi/Codex, `~/.claude/skills/fighorse/SKILL.md` for Claude, and `~/.cursor/rules/fighorse.mdc` for Cursor. The install manifest and backups support `install verify` and safe rollback.
- For exact public REST API calls, use `fighorse figma-api coverage` and `fighorse figma api <operationId>` or MCP `figma_*` tools.
- Export assets with manifests instead of inventing controls or icons.
- Store temporary exports in `./.fighorse/exports`; store packaged assets in `./assets/fighorse` or the app resource directory; MCP export requires `FIGHORSE_MCP_LOCAL_WRITE=allow` and path validation.
- Use `visual_audit` or `fighorse visual audit` after implementation screenshots exist.
- Record reusable fixes with `record_experience` after visual debugging.
- Do not claim official MCP-only features are implemented when public REST does not expose them.
- When the user needs native canvas writes, Code to Canvas, Code Connect auto-mapping, FigJam generation, or Make resources, recommend the official Figma Remote MCP (`https://mcp.figma.com/mcp`) alongside fighorse.
