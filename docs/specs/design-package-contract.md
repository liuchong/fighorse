# Design Package Contract

## Purpose

`fighorse design package` and MCP `get_design_package` create an AI-readable
package for implementing a concrete Figma target. The package must tell clients
whether the selected node is ready to implement or whether the user must choose
a narrower implementation node.

## Scope

Every package includes stable `scope` metadata:

- `status`: `ready_to_implement` or `needs_narrowing`.
- `reason`: `implementation_target`, `section`, `canvas`, `document`,
  `selection`, or `many_children`.
- `next_action`: an executable follow-up. For containers, the tool is
  `get_design_package` and the input should be a candidate node ID where
  `implementable=true`.

`FRAME`, `COMPONENT`, and `INSTANCE` targets are implementation targets.
`SECTION`, `CANVAS`, `DOCUMENT`, `SELECTION`, and broad multi-screen containers
require narrowing. fighorse must not silently replace a container with an
arbitrary child frame.

## Candidates

`screen_candidates[]` contains at most 20 candidate nodes from the target tree.
Each candidate includes:

- `id`, `name`, `type`, `width`, and `height`.
- `role`: `implementation_target` or `container`.
- `renderable`: whether Figma image rendering is expected to return a useful
  image URL.
- `implementable`: whether the node can be used as the primary implementation
  target.
- `reason`: short human-readable explanation.

`SECTION` candidates remain visible for compatibility, but they are containers:
`renderable=false` and `implementable=false`. Child frames/components/instances
are implementation targets.

## Screenshot Diagnostics

Screenshot count only includes non-null image URLs. If Figma returns `null` for
a requested node image, diagnostics report `null_count`, set package status to
`partial`, and guide clients to rerun `get_design_package` with an
implementable candidate node ID. Clients should not fall back to low-level
`get_node` for this container-narrowing case.

## Safety

The package remains read-only. It does not write to Figma, store design data, or
persist real project/file/node identifiers in shared guidance.
