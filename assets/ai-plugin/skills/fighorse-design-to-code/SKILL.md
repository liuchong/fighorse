---
name: fighorse-design-to-code
description: Use fighorse to turn Figma design context into implementation-ready code context.
---

# fighorse Design To Code

Use when implementing UI from a Figma file, frame, component, or selection URL.

## Flow

1. Parse the user target with `parse_figma_url`.
2. If the URL is a concrete design target, call `get_design_package`.
3. If the URL is a browser, team, or project page, use
   `get_resource_catalog` first and ask for a concrete file or node.
4. Use screenshots, tokens, component metadata, and Code Connect suggestions
   together. Do not treat any single source as complete truth.

## Boundaries

- Container nodes may need narrowing before implementation.
- Missing screenshots are diagnostics, not permission to hallucinate layout.
- Code Connect suggestions improve mapping but do not replace codebase checks.

