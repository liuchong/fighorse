# Figma Browser URL Boundaries

Reusable checks for Figma file-browser links:

- `/files/<browser-root>` is navigation context, not a design target. The
  identifier after `/files/` is not a team ID, and the public REST API cannot
  discover team IDs from this page.
- `/files/.../team/<team-id>` provides a usable team ID. Enumerate accessible
  resources with readonly `get_resource_catalog`; the same product-layer
  aggregation is available as `fighorse resource catalog`.
- `/project/<project-id>` and `/files/project/<project-id>` provide a usable
  project ID and can be cataloged directly without first discovering a team.
- The catalog defaults to projects, files, branches, and paginated team
  components/component sets/styles. Optional depth-1 file probes are bounded
  and return access state/page count rather than raw document trees.
- Project enumeration requires the `projects:read` token scope and may require
  Figma approval for Projects endpoints. HTTP 403 can also mean the token user
  cannot access the team; a successful `GET me` only proves the token is valid.
- Catalog status is `ready`, `partial`, or `blocked`. Preserve partial results
  and upstream `message`/`err`, but never copy arbitrary raw error bodies.
- Browser links still do not identify a frame or node. For design
  implementation, ask for a concrete file or selection URL after enumeration.
- Never store real browser IDs, team/project IDs, file keys, file names, node
  IDs, or URLs in reusable experience or shared diagnostics.
