# Canvas Write Contract

`fighorse` can write native Figma canvas content through a local companion
plugin. Public Figma REST remains read-oriented for document structure; native
canvas writes use the Figma Plugin API inside an open Figma file.

## Scope

- Supported editors: Figma Design (`figma`), FigJam (`figjam`), and Figma
  Slides (`slides`).
- Default mode: read-only. Write tools are hidden from MCP unless both
  `FIGHORSE_MCP_MODE=write` and `FIGHORSE_CANVAS_MODE=write` are set.
- Script execution is an explicit escape hatch. `canvas_execute_script` is
  unavailable unless `FIGHORSE_CANVAS_SCRIPT=allow` and the call includes
  `yes=true`.
- A Figma plugin session always represents the currently open file. If multiple
  sessions are connected, write calls must pass `session_id`; fighorse must not
  guess.

## Data Model

- `CanvasSession`: session id, plugin version, editor type, document name,
  current page, selection count, capabilities, and last heartbeat.
- `CanvasPlan`: transaction id, target session, expected editor, ordered
  operations, and verification options.
- `CanvasOperation`: stable operation name plus JSON arguments. Operation names
  are editor-gated.
- `CanvasResult`: status (`applied`, `rolled_back`, `partial`, `unknown`,
  `rejected`), operation results, created or changed node ids, and stable
  diagnostics.

## Security Rules

- The bridge binds to loopback only.
- Control HTTP calls require the local control secret written under
  `~/.fighorse/runtime`.
- Pairing codes are single-use, short-lived, and never logged after redemption.
- Figma tokens are not needed for local plugin writes and must not be copied into
  canvas bridge messages.
- Asset uploads are limited to approved local export roots.
- Unknown execution results are not retried automatically.

## BDD Acceptance

### Default Readonly

Given default environment variables
When an MCP client lists tools
Then canvas status and pairing tools are visible
And canvas write tools and script execution are hidden
And direct write calls return a policy error.

### Pairing

Given a fresh pairing code
When the plugin connects with a supported editor type
Then fighorse creates a session with editor capabilities
And reusing the pairing code is rejected.

### Ambiguous Session

Given two active canvas sessions
When a write plan omits `session_id`
Then fighorse returns `ambiguous_session`
And no command is sent to either plugin.

### Editor Gate

Given a FigJam session
When the plan contains the Design-only `create_page` operation
Then validation returns `editor_mismatch`
And the plugin receives no write command.

### Rollback

Given a session receives a plan with multiple operations
When an operation fails after earlier operations changed the canvas
Then the plugin triggers undo to the transaction boundary
And the result is `rolled_back` unless rollback itself fails.

### Unknown Result

Given a request was sent to the plugin
When the WebSocket disconnects before a result is received
Then fighorse returns `unknown`
And callers may inspect or verify, but must not automatically resend the plan.

### Script Gate

Given script mode is denied
When `canvas execute` or `canvas_execute_script` is called
Then no script is sent to the plugin.

Given script mode is allowed and the call includes `yes=true`
When a bounded script is executed
Then the result is size-limited and the operation has an undo boundary.

### Install Rollback

Given installation writes the canvas plugin bundle and service config
When a later install step fails
Then rollback only touches files mutated in that transaction
And existing credentials or unrelated client configuration remain unchanged.

