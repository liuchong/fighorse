# fighorse Design

`fighorse` is a Rust CLI and MCP server for turning Figma REST API data into implementation-grade context for humans and AI agents. It is public-first infrastructure: easy enough for a first-time user to reach a successful design package quickly, deep enough for teams and AI tools to build reproducible visual workflows over time.

## Product Goals

The project exists because the common Figma-to-AI paths each miss something important:

- Official Figma MCP is powerful, but black-box behavior makes debugging hard and future pricing or plan boundaries can affect availability.
- Screenshot-only prompting loses exact layout, color, typography, constraints, and component metadata.
- Raw Figma JSON is too large and noisy for LLM context windows.
- MCP-only community tools are useful inside an IDE but weak for scripts, CI, reproducible debugging, and non-MCP agents.

fighorse's goal is to provide a white-box data pipeline with three progressive layers:

- Simple first run: install, token, specific frame link, quickstart check, design package.
- Deep second run: asset manifests, exact REST dispatch, visual audit, project playbook.
- Long-term learning: local experience records that improve future AI runs without hidden memory.

The pipeline provides:

- Accurate Figma facts: node structure, dimensions, styles, layout, images, tokens, metadata.
- AI-ready context: compact, budget-aware, and explicit about assumptions.
- Visual references: screenshot/render URLs and local exported assets with manifests.
- Tool-neutral access: CLI first, MCP as an adapter, installable skills/rules for AI clients.
- Feedback memory: local experience storage so visual debugging lessons are reused.

## Core Principle: CLI Kernel, MCP Shell

The CLI is the primary product boundary. MCP exposes the same capabilities to AI tools but should stay thin.

```text
Figma REST API
  -> API modules + OpenAPI operation registry
  -> product layer: compact/filter/tokens/assets/design-package/visual-audit/playbook
  -> CLI output, files, manifests
  -> MCP tools/resources/prompts, AI clients, scripts, CI
```

This keeps the system:

- Public-first: users can succeed with the CLI before learning MCP.
- Inspectable: developers can run the same command an AI tool calls.
- Scriptable: shell, CI, and custom agents can use the binary directly.
- Transport-neutral: shared Streamable HTTP, explicit standard stdio MCP, and CLI all share the same business behavior.
- Easier to test: pure transformations and API wrappers remain separable from client configuration.

## Layered Architecture

```text
L4 Adapters
  mcp serve, resources/prompts, install client, install service, generated skills/rules

L3 Generated Context
  design package, visual audit, project playbook, markdown, tokens, tree, schema, manifests

L2 Transformations
  compact, filter, diff, diagnostics, experience matching, API coverage reports

L1 Figma API and Assets
  OpenAPI registry, operation dispatcher, files, nodes, images, comments, components, styles, variables, webhooks, downloads
```

L2 and L3 are the main differentiators. fighorse does not merely wrap REST endpoints; it reshapes Figma data into the form an AI agent can use without drowning in noise.

## Full REST Coverage

fighorse maintains an explicit OpenAPI operation registry for the public Figma REST snapshot. The registry currently tracks 48 operations and is used by:

- API wrappers in `src/fighorse/api`.
- Generic CLI dispatch: `fighorse figma api <operationId> --params '{...}'`.
- Generated official MCP tools: `figma_<operation_id_in_snake_case>`.
- Discovery and coverage reports: `fighorse figma-api coverage`.
- Contract tests that prevent missing or drifted endpoints.

The generic official layer is separate from product tools. Low-level REST tools preserve Figma semantics; product tools such as `get_design_package`, `visual_audit`, and `get_project_playbook` add AI workflow guidance on top.

## Design Package

`fighorse design package` and MCP `get_design_package` are the preferred high-level interface for design implementation. The package combines:

- Parsed Figma URL and selected target node.
- Compact structural context.
- Tokens and implementation hints.
- Figma render references and optional asset URLs.
- Platform and asset-format assumptions.
- Screen and component candidates.
- Export plan with CLI examples and MCP calls.
- Local learned experiences.
- Token confidence, missing-font diagnostics, and implementation risk checklist.
- Diagnostics and next-step warnings.

The package is designed to be both machine-readable and easy to inspect. It should tell an AI not only what to implement, but also what is missing before implementation is safe.

Important diagnostics include:

- Missing `platform`.
- Missing `asset_format`.
- Unsupported render format for Figma node rendering.
- Target node is a broad `CANVAS` or user-flow page.
- Context truncation due to token budget.
- Missing screenshots, tokens, or image fills.

`visual_audit` and `project playbook` extend the package into a full feedback loop. `visual_audit` turns a Figma URL plus optional app screenshot into a structured comparison checklist and experience suggestions. `project playbook` combines the AI contract, output policy, API coverage, and local lessons into reusable project instructions.

## Self-Discovery Contract

AI clients should not rely on stale prompt text. They should call:

- CLI: `fighorse discover --format json`
- MCP: `discover_fighorse`

The discovery manifest describes:

- Available CLI and MCP capabilities.
- REST coverage and official MCP comparison.
- Recommended design replication workflow.
- Safety defaults.
- Local write requirements.
- Experience-store behavior.
- Current AI contract.
- Installation and client configuration hints.
- MCP resources and prompts for clients that support them.

`doctor` complements discovery with runtime state: runtime information, auth status, home directory, MCP mode, local-write mode, and experience-store readiness.

## Self-Learning Model

fighorse keeps reusable lessons in append-only JSONL stores:

- Global: `~/.fighorse/experience/global.jsonl`.
- Project: `./.fighorse/experience.jsonl` after `fighorse install project`.
- Exact override: `FIGHORSE_EXPERIENCE_PATH`.
- Scope override: `FIGHORSE_EXPERIENCE_SCOPE=global|project|merged`.

This is intentionally local and transparent. The goal is not autonomous hidden memory; it is a project/user-owned log of practical design replication lessons.

Good records describe reusable patterns:

- Layout overlap caused by mapping repeated siblings into a stacking container.
- Compact component typography requiring direct inspection rather than global scaling.
- Device status bar or safe-area mismatch.
- Asset format or font availability constraints.

Records should not contain secrets, private absolute paths, or one-off project details.

## Safety Boundaries

fighorse separates three safety domains:

- Figma read access: requires a token and is the normal operating mode.
- Figma write access: disabled unless `FIGHORSE_MCP_MODE=write`.
- Local filesystem writes: disabled in MCP unless `FIGHORSE_MCP_LOCAL_WRITE=allow`.

Local file exports are still restricted to approved roots:

- `./.fighorse/exports`
- `./assets/fighorse`
- `~/.fighorse/exports`

This separation matters because downloading local screenshots or image fills is much less sensitive than mutating Figma, but still needs path validation. The user explicitly controls both domains.

The default MCP adapter is official Rust `rmcp` 2.2 `StreamableHttpService` with `LocalSessionManager`. It provides independent stateful sessions, validates Host and Origin before dispatch, supports JSON or event-stream responses under the standard Streamable HTTP contract, and cancels sessions during graceful SIGINT/SIGTERM shutdown. The event stream returned by `/mcp` is not the retired legacy SSE transport. `/sse` and `/messages` are not served, and `--transport sse` fails with migration guidance to HTTP. Explicit compatibility mode uses standard rmcp stdio without a private framing protocol.

## Installation Design

`fighorse install` generates reviewable artifacts by default and mutates user-level config only with `--apply`. The installer targets:

- Home/config setup under `~/.fighorse`.
- Binary install under `~/.fighorse/bin/fighorse`.
- Auth storage in `~/.fighorse/config.json`.
- MCP client snippets for Cursor, Codex, Kimi, Claude, opencode, and generic agents.
- User-level skills/rules for AI clients.
- macOS launchd and Linux systemd user services for Streamable HTTP MCP.

When native client commands are available, installer code prefers them. Otherwise it writes standard user configuration files with backups.

`fighorse install --default --apply` defaults to CLI-only because a public CLI should not surprise users with a background service or bound port. Long-running MCP service setup is explicit:

```bash
fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply
fighorse install client --client claude --apply
```

Installed clients share `http://127.0.0.1:9449/mcp`. Native payloads are Cursor `{"url":"http://127.0.0.1:9449/mcp"}`, Kimi `{"transport":"http","url":"http://127.0.0.1:9449/mcp"}`, Claude `{"type":"http","url":"http://127.0.0.1:9449/mcp"}`, and Codex `[mcp_servers.fighorse]` with the URL.

The service transaction is `preflight -> backup -> binary -> service -> health_ready -> clients -> skills -> verified`. Client files are not written until `/health` and a real `initialize` plus `tools/list` handshake succeed. The manifest records managed hashes, backup paths, order, and removals as `desired_absent: true`; rollback traverses those entries in reverse order and also restores service state. Customized legacy skills remain untouched and receive deterministic conflict backups.

Canonical targets collapse per-client copies into exactly three artifacts: `~/.agents/skills/fighorse/SKILL.md` for Cursor/Kimi/Codex, `~/.claude/skills/fighorse/SKILL.md` for Claude, and `~/.cursor/rules/fighorse.mdc` for Cursor. Fresh service and explicit stdio payloads deny local writes; migration preserves an existing explicit allow.

## Ecosystem Position

fighorse borrows lessons from existing tools while choosing a different boundary.

Official Figma MCP is prescriptive and deeply integrated. It can expose Code Connect, Code to Canvas, and design-system search, but its behavior is opaque and tied to Figma's product surface.

Framelink-style MCP is descriptive and lightweight. It gives AI layout and style facts instead of generated code structure. This avoids "poisoning" context with a framework decision the current codebase may not want.

fighorse defaults to the descriptive path: facts over generated code. It can still expose richer Figma metadata and write-capable endpoints when explicitly enabled, but the main workflow is "precise context in, project-native implementation out."

Official-only product surfaces that are not in the public REST OpenAPI are marked as unsupported by public REST rather than silently approximated. Examples include native canvas mutation, Code to Canvas, automatic Code Connect mapping discovery, Make resources, and FigJam generation. fighorse may offer open alternatives such as user-maintained component maps or generic resource ingestion, but it should not pretend to implement private Figma product APIs.

## Data Fidelity

Figma REST API data is generally specification-faithful for implementation facts:

- Geometry, dimensions, bounds, and coordinates.
- Colors, effects, strokes, fills, corner radii.
- Text characters and style metadata.
- Auto Layout properties.
- Component and instance relationships.
- Render URLs and image fills.

Pixel-perfect browser or mobile rendering still requires a feedback loop because rendering engines, font availability, antialiasing, blend modes, and system UI differ from Figma. fighorse therefore combines structured JSON with screenshots and insists on visual verification rather than claiming first-pass perfection.

## Lessons From Field Use

A real Android Compose prototype exposed several practical requirements:

- `design package` should normalize Figma URLs and node ids.
- `smoke` and `diagnostics.status=ready` are useful readiness signals.
- Export manifests are more reliable than terminal logs for downstream scripts.
- Safe filenames such as `376_12995.png` work better across app build systems.
- Broad flow nodes should be narrowed to concrete frames before implementation.
- Repeated rows need list/linear containers; generic stack containers cause overlap.
- Compact message cards and full chat cards can require different typography.
- Mobile screens need scroll-safe layouts.
- Font availability and status-bar/safe-area behavior must be handled explicitly.
- AI tools need platform and asset-format choices before implementation.

These lessons directly shaped the design package, export manifest, local-write, and experience-store contracts.

## Non-Goals

fighorse does not aim to be a general code generator. Code should be produced by the AI agent using the target repository's existing patterns.

It also does not aim to replace Figma's own product integrations. Official MCP remains appropriate for Code Connect, Code to Canvas, FigJam generation, or native Figma mutation workflows.

The durable product boundary is the context and asset pipeline: fetch, compact, explain, export, verify, and learn.

## Verification Strategy

The project should remain verifiable without a real Figma token:

- Unit and integration tests cover argument parsing, compacting, URL parsing, path validation, OpenAPI coverage, operation dispatch, MCP tool/resource routing, official HTTP and standard stdio transports, discovery manifest, transactional installation, design-package diagnostics, and documentation consistency.
- Integration tests that touch the real Figma API are opt-in.
- Docs and generated install artifacts must reflect current CLI behavior.

The guiding maintenance rule is consistency across CLI, MCP schemas, installer output, discovery manifest, skills/rules, and formal docs.
