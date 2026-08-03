# Resource Catalog Contract

## Purpose

`fighorse resource catalog` and MCP `get_resource_catalog` turn a Figma team
or project browser link into a bounded, read-only catalog of accessible design
resources. The catalog is navigation context; it does not replace a concrete
file or selection URL for implementation.

## Sources

Exactly one source is resolved in this order:

1. Explicit `team_id`.
2. Explicit `project_id`.
3. A Figma URL containing `/team/<team-id>`.
4. A Figma URL containing `/project/<project-id>` or
   `/files/project/<project-id>`.

`/files/<browser-root>` is not a team source. The public REST API cannot
discover teams from it, so the result is `blocked` without making a network
request. A design/file/proto/board URL is also not a catalog source.

## Request

- `include_libraries` defaults to `true`.
- `branch_data` defaults to `true`.
- `probe_file_access` defaults to `false`.
- `max_probes` defaults to `25`; `0` means unlimited only when probes are
  explicitly enabled.

The catalog first performs an anonymous auth probe with `GET me`; no user
identity fields are returned. Team sources then call `GET team projects`,
followed by `GET project files` for every project. Project sources call only
`GET project files`.

When libraries are enabled, team components, component sets, and styles are
read through every `meta.cursor.after` page. Repeated cursors stop the stream
and produce a partial result. File probes are sequential `GET file` requests
with `depth=1`; they return only access state and top-level page count.

## Response

The stable response kind is `fighorse.resource-catalog.v1`.

- `status`: `ready`, `partial`, or `blocked`.
- `source`: source kind and resolved team/project identifiers.
- `request`: effective request switches and limits.
- `auth_probe`: boolean success only.
- `summary`: project, file, branch, library, and probe counts.
- `projects[].files[]`: file key, name, update/thumbnail metadata, branch
  metadata, and optional probe result.
- `team_library`: component, component-set, and style collections plus status.
- `diagnostics[]`: stable code, scope, HTTP status, safe Figma message, and
  actionable next step.
- `next_tools[]`: existing per-file tools for full content and asset work.

No response, diagnostic, reusable experience, fixture, or test log may contain
a real token. Shared diagnostics and experience must also omit real team,
project, file, and node identifiers.

## Status and Failure Rules

- `blocked`: no primary project/file catalog can be obtained.
- `partial`: the primary catalog exists, but a project, library stream, file
  probe, timeout, rate limit, or configured bound prevented full completion.
- `ready`: every requested stream completed without truncation or failure.

`GET me` failure blocks all subsequent requests. A successful auth probe
followed by Projects HTTP 403 reports possible `projects:read`, Projects
limited-access eligibility, or team-access causes without claiming which one
is certain. Library and file-probe 403 diagnostics name
`team_library_content:read` and `file_content:read` respectively.

HTTP 429 stops only the affected stream and preserves prior results. The
catalog does not retry indefinitely. Safe upstream `message` or `err` strings
may be included; arbitrary raw bodies are not included.

CLI prints the report as JSON. `blocked` exits non-zero; `partial` exits zero
with explicit status. MCP returns the same report as a successful read-only
tool result; malformed or conflicting inputs are tool errors.

## Persistence and Safety

The command writes nothing by default. CLI `--output` is the only catalog
write path and may contain private design metadata, so generated catalogs must
not be committed. MCP needs neither Figma write mode nor local-write mode.

There is no cache, database, background crawl, resume token, or migration in
v1. Full document trees and asset downloads remain explicit per-file
operations through existing tools.

## Required Scopes

- `current_user:read` for the auth probe.
- `projects:read` for team/project enumeration.
- `team_library_content:read` for team libraries.
- `file_content:read` for optional file probes.

Scopes do not override team or file access. Projects endpoints are limited
access and are unavailable to public OAuth apps.
