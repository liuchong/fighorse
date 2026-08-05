# Canvas Plugin Boundaries

Reusable checks for native canvas writes through the local fighorse plugin
bridge:

- Treat every plugin connection as an explicit open-file session. If more than
  one session is connected, require `session_id`; never guess from a file name.
- Public REST token readiness and canvas session readiness are separate. Canvas
  writes need the local bridge and paired plugin, not `FIGMA_TOKEN`.
- Keep structured `CanvasPlan` operations as the default. Canvas writes require
  `FIGHORSE_CANVAS_MODE=write`. Use arbitrary Plugin API JavaScript through
  `canvas_execute_script` only when `FIGHORSE_CANVAS_SCRIPT=allow` is enabled
  and the exact call is confirmed.
- If a transaction returns `unknown` after timeout or disconnect, do not resend
  it automatically. Inspect or verify first to avoid duplicate creates/deletes.
- Keep editor gates explicit: Figma Design, FigJam, and Slides share protocol
  names but not every operation.
- Do not store real file names, node IDs, pairing codes, or design content in
  reusable experience.

