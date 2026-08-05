---
name: fighorse-code-connect
description: Use fighorse native Code Connect support to generate, validate, preview, publish, and unpublish mappings.
---

# fighorse Code Connect

Use when mapping Figma components to code components.

## Flow

1. Generate or parse Code Connect templates with fighorse commands.
2. Validate templates before preview or publish.
3. Preview against Figma before publishing.
4. Treat publish and unpublish as remote writes.

## Permission Gates

- MCP Code Connect writes require explicit Code Connect permission.
- Do not execute repository-provided `.figma.ts` or `.figma.js` locally.
- Reject project-configured API URLs that could exfiltrate a token.

## Boundaries

- Native support tracks observed Figma Code Connect behavior and may need
  compatibility updates if Figma changes private protocol details.

