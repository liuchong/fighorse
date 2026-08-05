---
name: fighorse-resource-catalog
description: Use fighorse to enumerate accessible Figma teams, projects, files, and design system libraries.
---

# fighorse Resource Catalog

Use when the user gives a Figma browser, team, or project URL instead of a
concrete design target.

## Flow

1. Parse the URL with `parse_figma_url`.
2. For project or team contexts, call `get_resource_catalog`.
3. Preserve `ready`, `partial`, and `blocked` status in the response.
4. Ask for a concrete file or node URL before design implementation.

## Boundaries

- A Figma file browser page is navigation context, not a design target.
- A valid token does not prove access to every team or project.
- Preserve partial results; do not discard accessible resources because one
  project or library failed.

