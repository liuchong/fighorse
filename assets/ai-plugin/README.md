# fighorse AI Plugin Bundle

This directory is the source template for local fighorse AI client bundles.

The generated bundle is local-only and is not a public marketplace package. It
combines:

- MCP configuration for the local fighorse HTTP service.
- Cursor, Claude, Gemini, and generic MCP manifests.
- Shared workflow skills.
- Safety defaults that keep write permissions disabled unless explicitly
  enabled by fighorse configuration and per-call confirmation.

Generated bundles must not contain Figma tokens, exported design data, runtime
session state, pairing codes, or node IDs.

