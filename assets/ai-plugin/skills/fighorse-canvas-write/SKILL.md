---
name: fighorse-canvas-write
description: Use fighorse local canvas bridge to create, modify, verify, and undo native Figma nodes.
---

# fighorse Canvas Write

Use when the user wants fighorse to draw or modify Figma Design, FigJam, or
Slides content.

## Flow

1. Ensure the local canvas bridge is installed and running.
2. Pair the Figma plugin with `fighorse canvas pair`.
3. Use structured `CanvasPlan` operations first.
4. Pass an explicit `session_id` whenever multiple sessions are connected.
5. Verify the transaction before making follow-up edits.

## Permission Gates

- Structured writes require `FIGHORSE_CANVAS_MODE=write`.
- MCP writes also require `FIGHORSE_MCP_MODE=write` and per-call `yes=true`.
- Arbitrary Plugin API JavaScript requires `FIGHORSE_CANVAS_SCRIPT=allow` and
  per-call confirmation.

## Boundaries

- Never automatically retry a transaction with status `unknown`.
- Do not store pairing codes, node IDs, file names, or design content in shared
  experience.

