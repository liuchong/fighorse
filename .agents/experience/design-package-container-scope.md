# Design Package Container Scope

Reusable checks for broad Figma design-package targets:

- `FRAME`, `COMPONENT`, and `INSTANCE` are implementation targets. Their package
  `scope.status` should be `ready_to_implement` when screenshots and tokens are
  usable.
- `SECTION`, `CANVAS`, `DOCUMENT`, and `SELECTION` are containers. Their package
  `scope.status` should be `needs_narrowing`.
- When narrowing is required, choose `screen_candidates[].id` where
  `implementable=true` and rerun `get_design_package`. Do not silently replace
  the user's target with an arbitrary child.
- Keep SECTION candidates in `screen_candidates` for compatibility, but mark
  them as `role=container`, `renderable=false`, and `implementable=false`.
- Count only non-null screenshot URLs. `diagnostics.screenshots.null_count > 0`
  means Figma returned a null render; narrow to an implementable candidate
  instead of falling back to low-level `get_node`.
- Shared experience must use placeholder file and node IDs only.
