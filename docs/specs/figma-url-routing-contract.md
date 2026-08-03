# Figma URL Routing Contract

## Purpose

`fighorse url parse` and MCP `parse_figma_url` turn a user supplied Figma value
into a machine-readable routing decision. The parser does not call Figma and
does not infer team or project identifiers that are not present in the input.

## Stable Fields

Existing fields remain stable: `valid`, `kind`, `file_key`, `node_id`,
`raw_node_id`, `browser_root_id`, `team_id`, `project_id`, `embedded_url`, and
`error`.

The parser also emits additive routing fields:

- `url_role`: `browser_root`, `team_browser`, `project_browser`,
  `design_target`, `file_key`, or `unknown`.
- `catalog_eligible`: `true` only when `team_id` or `project_id` is available
  and no design file key is present.
- `design_target`: `true` only when a file key or raw file key can be used by
  design-context tools.
- `next_action`: the safest next tool or user request.
- `error_code`: a stable machine code for invalid or blocked routes when one is
  known.

## Routing Rules

- `/files/<browser-root>` is navigation context only. The public Figma REST API
  cannot enumerate teams from this value, so `catalog_eligible=false`,
  `design_target=false`, `next_action=ask_for_team_or_file_url`, and
  `error_code=browser_root_not_enumerable`.
- `/files/.../team/<team-id>` is catalog context. It is not a design target, but
  it is eligible for `get_resource_catalog`.
- `/project/<project-id>` and `/files/project/<project-id>` are catalog context
  and route to `get_resource_catalog`.
- `/design/<file-key>`, `/file/<file-key>`, `/proto/<file-key>`, and
  `/board/<file-key>` route to `get_design_package`. If the URL has no
  `node-id`, AI clients should ask the user to narrow to a frame, component, or
  group before implementation.
- A raw non-URL string is treated as a file key and routes to
  `get_design_package`.

## Safety

The parser must not store, log, or serialize real tokens. Shared fixtures and
experience records must use placeholder browser roots, team IDs, project IDs,
file keys, and node IDs.
