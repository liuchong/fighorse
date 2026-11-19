---
name: fighorse-resource-catalog
description: Use fighorse to enumerate accessible Figma teams, projects, files, and design system libraries.
---

# fighorse Resource Catalog

Use when the user gives a Figma browser, team, folder, or project URL instead of
a concrete design target.

## Flow

1. Parse the URL with `parse_figma_url`.
2. For folder, project, or team contexts, call `get_resource_catalog` (traverses
   nested v2 folders and tolerates Plan Access Token 403 on `/v1/me`).
3. Use `search_design_system` when searching components, styles, or variables.
4. Preserve `ready`, `partial`, and `blocked` status in the response.
5. Ask for a concrete file or node URL before design implementation.

## Boundaries

- A Figma file browser page is navigation context, not a design target.
- A valid token does not prove access to every team or project.
- Preserve partial results; do not discard accessible resources because one
  project or library failed.

