---
name: fighorse-code-connect
description: Use fighorse native Code Connect support to generate, validate, preview, publish, and unpublish mappings.
---

# fighorse Code Connect

Use when mapping Figma components to code components.

## Flow

1. Generate or parse Code Connect v2.1.0 parserless templates (including `SLOT`
   `getSlot` mappings and `.figma.batch.json` single-template resolution) with
   fighorse commands.
2. Validate templates before preview or publish.
3. Preview against Figma (`--unique` to deduplicate combinations) before publishing.
4. Treat publish and unpublish as remote writes; inspect `diagnostics.pro_plan_restrictions`
   when publishing on Professional plans.

## Permission Gates

- MCP Code Connect writes require explicit Code Connect permission.
- Do not execute repository-provided `.figma.ts` or `.figma.js` locally.
- Reject project-configured API URLs that could exfiltrate a token.

## Boundaries

- Native support tracks observed Figma Code Connect behavior and may need
  compatibility updates if Figma changes private protocol details.

