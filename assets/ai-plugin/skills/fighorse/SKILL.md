---
name: fighorse
description: Entry skill for using fighorse MCP, discovery, install guidance, and workflow-specific fighorse skills.
---

# fighorse

Use this entry skill before working with Figma through fighorse.

## Required Flow

1. Check readiness with `discover_fighorse` or `fighorse doctor --format json`.
2. Confirm the target workflow and load the matching workflow skill:
   - `fighorse-design-to-code`
   - `fighorse-canvas-write`
   - `fighorse-resource-catalog`
   - `fighorse-code-connect`
   - `fighorse-self-learning`
3. Keep Figma REST token readiness, local MCP readiness, canvas plugin
   readiness, and local file write permission separate.

## Boundaries

- Do not assume canvas write access from a Figma REST token.
- Do not retry unknown canvas transactions automatically.
- Do not store real file names, node IDs, screenshots, pairing codes, or tokens
  in reusable memory.
- Prefer the local HTTP MCP service at `http://127.0.0.1:9449/mcp`.

