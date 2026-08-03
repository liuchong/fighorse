# fighorse Agent Instructions

- Start with `fighorse discover --format json` or MCP `discover_fighorse`.
- Then call `check_fighorse_ready` or `fighorse doctor --format json`. If `auth.has_token` is false, tell the user to run `fighorse auth login --token <FIGMA_TOKEN>` or set `FIGMA_TOKEN`; do not retry Figma API calls until setup is fixed.
- Load local lessons with `list_experiences` before using `get_design_package`.
- Use `platform` and `asset_format` explicitly; ask if unknown.
- Prefer the installed shared HTTP MCP endpoint `http://127.0.0.1:9449/mcp`; avoid starting duplicate long-lived stdio servers unless a client requires compatibility mode.
- If an expected tool is missing after install, upgrade, or service restart, reconnect the MCP client and request `tools/list` again before deciding the feature is unavailable; clients can cache old tool lists for an active session.
- The shared endpoint uses official Rust `rmcp` 2.2 Streamable HTTP with independent stateful sessions, Host/Origin validation, JSON or event-stream response negotiation, and graceful shutdown. Legacy `/sse` and `/messages` endpoints are retired; `--transport sse` must fail and migrate to HTTP.
- Install Cursor, Codex, Kimi, and Claude with `fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply`; install Claude alone with `fighorse install client --client claude --apply`. Service activation and `/health` plus `initialize`/`tools/list` verification happen before client configuration.
- Canonical instruction targets are `~/.agents/skills/fighorse/SKILL.md` for Cursor/Kimi/Codex, `~/.claude/skills/fighorse/SKILL.md` for Claude, and `~/.cursor/rules/fighorse.mdc` for Cursor. The install manifest and backups support `install verify` and safe rollback.
- For exact public REST API calls, use `fighorse figma-api coverage` and `fighorse figma api <operationId>` or MCP `figma_*` tools.
- Treat Figma file-browser links as navigation context, not design targets. `parse_figma_url` extracts `team_id` from `/files/.../team/<team-id>`; use `get_team_projects`, then `get_project_files` for every project. Enumeration requires `projects:read` and may require Figma Projects endpoint approval; HTTP 403 can also mean the token user lacks team access. `/files/<browser-root>` alone cannot discover teams through the public REST API. Keep browser IDs, team/project IDs, file keys, names, and URLs out of reusable experience.
- For modern Code Connect templates, use `fighorse code-connect generate|parse|validate|preview|publish|unpublish` or MCP `parse_code_connect_template`, `validate_code_connect`, `preview_code_connect`, `publish_code_connect`, and `unpublish_code_connect`. MCP readonly mode exposes parse, validate, and preview tools; publish/unpublish appear only when `FIGHORSE_MCP_MODE=write`. MCP preview/publish requires `FIGHORSE_MCP_CODE_CONNECT=allow`; publish/unpublish also requires `FIGHORSE_MCP_MODE=write`. Automatic Code Connect mapping remains an official Figma Remote MCP workflow.
- Export assets with manifests instead of inventing controls or icons.
- Store temporary exports in `./.fighorse/exports`; store packaged assets in `./assets/fighorse` or the app resource directory; MCP export requires `FIGHORSE_MCP_LOCAL_WRITE=allow` and path validation.
- Use `visual_audit` or `fighorse visual audit` after implementation screenshots exist.
- Record reusable fixes with `record_experience` after visual debugging.
- Do not claim official MCP-only features are implemented when public REST does not expose them.
- When the user needs native canvas writes, Code to Canvas, Code Connect auto-mapping, FigJam generation, or Make resources, recommend the official Figma Remote MCP (`https://mcp.figma.com/mcp`) alongside fighorse.
