# MCP and AI Client Installation Contract

Status: accepted for implementation

## Purpose

This contract defines the Rust implementation of fighorse MCP transports and
the installation lifecycle for Cursor, Kimi, Claude, and Codex. It replaces
the incomplete behavioral-parity claim made during the ClojureScript-to-Rust
rewrite with explicit, executable behavior.

## Transport decisions

- Streamable HTTP is the default shared transport at
  `http://127.0.0.1:9449/mcp`.
- Standard Streamable HTTP covers request/response and streaming behavior on
  the same MCP endpoint. The legacy HTTP+SSE endpoints `/sse` and `/messages`
  are retired.
- `--transport sse` is not an alias. It fails with a migration message that
  points clients to `--transport http` and `/mcp`.
- stdio remains an explicit compatibility transport. Installed clients do not
  spawn separate stdio servers while the shared HTTP service is available.
- `/health`, `/manifest`, and `/discover` are product endpoints and never
  substitute for MCP protocol responses.

## MCP behavior

The server uses the official Rust MCP SDK for protocol lifecycle and message
handling. Existing fighorse tool, resource, prompt, and policy modules remain
the only business-capability source.

The HTTP server:

1. validates allowed Origin values before processing MCP requests;
2. negotiates a supported MCP protocol version during initialization;
3. accepts repeated independent client initialization;
4. supports JSON or SSE responses according to Streamable HTTP negotiation;
5. drains active work during SIGINT or SIGTERM before releasing the singleton
   lock;
6. never returns the fighorse discovery manifest from `GET /mcp`.

## Installation model

An installation is an ordered transaction:

`preflight -> backup -> binary -> service -> health_ready -> clients -> skills -> verified`

The canonical plan contains:

- binary source, target, and PATH links;
- service manager, command, endpoint, and local-write policy;
- client-native MCP payloads;
- canonical skill and rule targets;
- managed-file hashes and backup locations.

The transaction report contains the immutable plan, completed stages,
verification results, skill-migration results, and rollback checks. It never
contains a Figma token.

`write_managed` records the previous content once, writes atomically, and
keeps a stable write order. JSON and TOML client configuration is merged so
unknown user-owned fields survive. A first manifest commit records all
managed changes; manifest, binary, permission, and optional service checks
then run; a final commit stores those checks as `last_verification`.
Aggregate client files are verified semantically at the fighorse MCP entry,
not by whole-file hash, because clients may update unrelated runtime metadata.
All dedicated fighorse files continue to require exact content and file type.

If a write or service step fails before commit, pending files are restored in
reverse order and a newly activated service is unloaded or the previous
service is re-enabled. If verification fails after commit, the committed
manifest drives the same managed-file rollback and service rollback.
Rollback never overwrites a managed file whose current hash differs from the
manifest, and never removes unrelated files.

## Client payloads

- Cursor: `{"url":"http://127.0.0.1:9449/mcp"}`.
- Kimi: `{"transport":"http","url":"http://127.0.0.1:9449/mcp"}`.
- Claude: `{"type":"http","url":"http://127.0.0.1:9449/mcp"}`.
- Codex: `[mcp_servers.fighorse]` with the URL.

Review artifacts and applied configurations are rendered from the same
client payload function.

## Skill and rule targets

- Cursor, Kimi, and Codex share
  `~/.agents/skills/fighorse/SKILL.md`.
- Claude uses `~/.claude/skills/fighorse/SKILL.md`.
- Cursor additionally uses `~/.cursor/rules/fighorse.mdc`.

Legacy generated copies are removed only when their content hash matches a
known fighorse-generated artifact. Modified files are backed up and reported
as conflicts.

Migration inspects the legacy `fighorse` skill directories under
`~/.cursor/skills`, `~/.codex/skills`, `~/.kimi/skills`, and
`~/.config/agents/skills`. It also inspects obsolete generated `AGENTS.md`
and `cursor-rule.mdc` files beside the canonical shared and Claude skills.
Managed removals are persisted with `desired_absent: true`; verification
requires those paths to remain absent, while rollback restores their managed
backups only if they are still absent. If a user recreates a removed path,
rollback reports a conflict and does not overwrite it. A previously managed
path already absent is converted to `desired_absent` without inventing a
backup. Conflict backups for customized legacy files are deterministic and
are not removal entries, so repeated migration neither deletes the customized
file nor creates duplicate copies.

## Security and migration

- Token configuration is merged, never replaced, and is mode `0600` on Unix.
- Figma write mode and local export write mode remain independent.
- A fresh service install denies local export writes unless explicitly
  enabled. Existing explicit `allow` state is preserved during migration.
- Explicit stdio payloads also default to local-write `deny`.
- Export paths remain restricted to project `.fighorse/exports`,
  `assets/fighorse`, and the user fighorse export root.
- Legacy SSE configuration receives an actionable migration error; no
  half-working endpoint is advertised.

## BDD acceptance scenarios

### Service becomes ready before client registration

Given a clean HOME and no process listening on the configured port  
When service mode installs Cursor, Kimi, Claude, and Codex  
Then the binary and service are installed first  
And an initialize plus tools/list handshake succeeds  
And only then are client MCP configurations written.

### Repeated Streamable HTTP initialization

Given a running shared service  
When two clients initialize independently and call tools/list  
Then both receive valid MCP responses  
And neither observes state from the other session.

### Protocol endpoint separation

Given a client requests `/mcp` with Streamable HTTP Accept headers  
When the request is a valid MCP request  
Then the response is a valid MCP JSON or SSE response  
And it is never the fighorse product manifest.

### Origin protection

Given an HTTP MCP request with an unapproved Origin  
When the server receives the request  
Then it responds with HTTP 403 without dispatching a tool.

### Legacy SSE migration

Given a user selects `--transport sse`  
When fighorse parses the command  
Then it exits unsuccessfully  
And the error names `--transport http` and `/mcp`.

### CLI-only installation

Given no explicit service mode  
When a user runs the default install command  
Then no service is configured or started  
And no AI client MCP configuration is written.

### Transaction rollback

Given managed files have backups  
When a later installation step fails  
Then completed managed writes are restored in reverse order  
And unrelated user configuration remains unchanged.

### Skill deduplication

Given legacy generated skill copies in several client directories  
When installation migrates skills  
Then matching generated copies converge to canonical targets  
And a modified copy is preserved, backed up, and reported.

### Auth file safety

Given config contains token and unknown future fields  
When auth is updated  
Then unknown fields remain present  
And the file mode is `0600`  
And command output contains no token.

### Independent write gates

Given Figma write mode is readonly and local-write mode is denied  
When a Figma mutation or local export is requested  
Then each is rejected by its own policy  
And an export outside approved roots is always rejected.

## Verification contract

Before deployment:

- `cargo fmt --check`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`
- `git diff --check`
- temporary-HOME install, verification, and rollback tests

After a verified commit:

- install the release binary and service;
- verify the HTTP MCP handshake;
- run `kimi mcp test fighorse`;
- run Claude MCP health inspection;
- verify Codex configuration and a fighorse tool call;
- reload Cursor MCP and call `discover_fighorse` and
  `check_fighorse_ready`.

## Non-goals

- Restoring `/sse`, `/messages`, or legacy SSE connection accounting.
- Implementing Figma-native canvas mutation or other official Figma MCP-only
  features.
- Restoring Bun-specific polling or explicit-exit workarounds.
- Advertising remote TLS, auth, persistent event stores, or Windows service
  management before those capabilities exist.
