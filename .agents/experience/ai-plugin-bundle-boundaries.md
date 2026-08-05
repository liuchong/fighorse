# AI Plugin Bundle Boundaries

Reusable rule: keep AI client distribution packages as local-only artifacts
backed by shared source templates.

- Source templates belong under `assets/ai-plugin/`; generated packages belong
  under `~/.fighorse/ai-plugin/fighorse/`.
- Client manifests should be thin wrappers over shared MCP config and workflow
  skills.
- Installing the bundle must not enable write permissions. Figma writes, local
  file export, canvas writes, Plugin API JavaScript, and Code Connect publish
  remain controlled by fighorse gates and per-call confirmation.
- Do not store tokens, real file names, node IDs, pairing codes, screenshots, or
  exported design data in reusable skills or experience.
- If a client's automatic install path is not verified, generate artifacts and
  report manual actions instead of guessing private config directories.

