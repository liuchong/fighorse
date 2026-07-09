# fighorse

> [English](README.md) | [中文](../zh/README.md) | [Русский](../ru/README.md)

The Swiss Army knife for Figma data, shaped for AI consumption.

`fighorse` is a Rust CLI and MCP Server. It does not generate code; instead, it transforms Figma REST API data into stable, consumable context for both AI programming tools and developers: the complete public REST API, structural trees, compact JSON, screenshot URLs, design tokens, image/component exports, manifests, self-discovery info, and local experience.

The core philosophy is **CLI as kernel, MCP as shell**. The CLI stays white-box, scriptable, and debuggable; MCP lets Cursor, Codex, Kimi, Claude, opencode, and other AI tools call the same capabilities directly.

## Quick Start

The default path is CLI-only. It does not start a long-running MCP service or bind any port.

```bash
cargo build --release
./target/release/fighorse install --default --apply --source ./target/release/fighorse
```

```bash
fighorse auth login --token <FIGMA_TOKEN>
fighorse quickstart
```

In Figma, copy a link to the exact frame, component, or group you want to inspect. Avoid starting from a whole page/canvas unless you are exploring.

```bash
fighorse quickstart "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
```

Generate the context package needed to replicate the design:

```bash
fighorse design package "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>" \
  --platform <target-platform> \
  --asset-format <asset-format>
```

Export visual assets:

```bash
fighorse image export <file_key> --ids 1:2,1:3 --dir ./.fighorse/exports --manifest
fighorse component export <file_key> --ids 2:8 --format svg --dir ./assets/fighorse --manifest
fighorse asset download <file_key> --dir ./assets/fighorse --manifest
```

Optional MCP service mode for AI clients:

```bash
fighorse install --default --mode service --clients cursor,codex,kimi --apply
```

Package distributable binaries with Cargo. Cross-compile per target with the
matching Rust toolchain (or `cargo-zigbuild` for Linux targets):

```bash
cargo build --release
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

## Documentation

- [Quickstart](quickstart.md): first successful CLI run, frame link, design package, optional MCP setup.
- [User Guide](user-guide.md): install, auth, CLI, MCP service, local asset export, experience storage, troubleshooting.
- [AI Client Guide](ai-client-guide.md): how AI tools should self-discover, call MCP/CLI, export assets, ask for platform/asset format, and record reusable lessons.
- [Design](design.md): architecture, product goals, ecosystem tradeoffs, self-discovery/self-learning model, safety boundaries.

## Core Commands

| Area | Commands |
|------|----------|
| Discovery | `discover`, `doctor`, `smoke`, `url parse`, `mcp config` |
| Official REST | `figma-api coverage`, `figma api <operationId>` |
| Design Package | `design package`, `visual audit`, `project playbook`, `experience summary`, `experience add` |
| Figma Data | `file get`, `file nodes`, `node get`, `file tree`, `file compact` |
| Assets | `image export`, `component export`, `asset download`, `images render`, `images fills` |
| Design System | `components`, `component-sets`, `styles`, `variables`, `tokens extract` |
| Install | `install`, `install self`, `install home`, `install auth`, `install binary`, `install client`, `install service`, `install skill`, `install all` |
| MCP | `mcp serve --transport http`, `mcp serve --transport sse --host 127.0.0.1`, explicit `stdio` compatibility mode |

## Safety Defaults

- Figma writes are disabled unless `FIGHORSE_MCP_MODE=write`.
- MCP local file exports require `FIGHORSE_MCP_LOCAL_WRITE=allow`.
- Export paths are limited to `./.fighorse/exports`, `./assets/fighorse`, and `~/.fighorse/exports`.
- Installed AI clients default to the shared local HTTP MCP endpoint at `http://127.0.0.1:9449/mcp`; the MCP server uses a singleton lock to avoid duplicate long-running processes.
- Normal CLI commands remain one-shot processes: they do not start the MCP service, bind ports, or use the MCP singleton lock. `fighorse install all` defaults to CLI-only setup; use `--mode service` or `install service --apply` only when you explicitly want a long-running MCP service.
- AI clients must ask for the target platform and asset format when missing; PNG is only a render fallback, not a product decision.

## Development

```bash
cargo test
cargo build --release
cargo clippy
```

Real Figma API tests are opt-in:

```bash
FIGMA_INTEGRATION_TESTS=1 FIGMA_TOKEN=<token> cargo test -- --ignored
```

## License

[1st Public License (1PL)](https://license.pub/1pl/)
