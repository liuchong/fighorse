# fighorse

Figma 数据的瑞士军刀，专门打磨成 AI 最容易消化的形状。

`fighorse` 是一个 Bun-first 的 ClojureScript CLI 和 MCP Server。它不生成代码，而是把 Figma REST API 数据整理成 AI 编程工具和开发者都能稳定消费的上下文：完整公开 REST API、结构树、精简 JSON、截图 URL、设计 token、图片/控件导出、manifest、自发现信息和本地经验。

核心理念是 **CLI 为核，MCP 为壳**。CLI 保持白盒、可脚本化、可调试；MCP 让 Cursor、Codex、Kimi、Claude、opencode 等 AI 工具直接调用同一套能力。

## Quick Start

```bash
bun install
bun run build
bun run compile
./dist/fighorse --help
```

```bash
fighorse auth login --token <FIGMA_TOKEN>
fighorse discover --format json
fighorse doctor --format json
fighorse smoke "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
fighorse figma-api coverage --format json
```

生成复刻设计所需的上下文包：

```bash
fighorse design package "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>" \
  --platform <target-platform> \
  --asset-format <asset-format>
```

导出视觉资产：

```bash
fighorse image export <file_key> --ids 1:2,1:3 --dir ./.fighorse/exports --manifest
fighorse component export <file_key> --ids 2:8 --format svg --dir ./assets/fighorse --manifest
fighorse asset download <file_key> --dir ./assets/fighorse --manifest
```

## Documentation

- [User Guide](docs/user-guide.md): install, auth, CLI, MCP service, local asset export, experience storage, troubleshooting.
- [AI Client Guide](docs/ai-client-guide.md): how AI tools should self-discover, call MCP/CLI, export assets, ask for platform/asset format, and record reusable lessons.
- [Design](docs/design.md): architecture, product goals, ecosystem tradeoffs, self-discovery/self-learning model, safety boundaries.

## Core Commands

| Area | Commands |
|------|----------|
| Discovery | `discover`, `doctor`, `smoke`, `url parse`, `mcp config` |
| Official REST | `figma-api coverage`, `figma api <operationId>` |
| Design Package | `design package`, `visual audit`, `project playbook`, `experience summary`, `experience add` |
| Figma Data | `file get`, `file nodes`, `node get`, `file tree`, `file compact` |
| Assets | `image export`, `component export`, `asset download`, `images render`, `images fills` |
| Design System | `components`, `component-sets`, `styles`, `variables`, `tokens extract` |
| Install | `install home`, `install auth`, `install binary`, `install client`, `install service`, `install skill`, `install all` |
| MCP | `mcp serve --transport stdio`, `mcp serve --transport sse --host 127.0.0.1` |

## Safety Defaults

- Figma writes are disabled unless `FIGHORSE_MCP_MODE=write`.
- MCP local file exports require `FIGHORSE_MCP_LOCAL_WRITE=allow`.
- Export paths are limited to `./.fighorse/exports`, `./assets/fighorse`, and `~/.fighorse/exports`.
- AI clients must ask for the target platform and asset format when missing; PNG is only a render fallback, not a product decision.

## Development

```bash
bun run test
bun run build
bun run compile
bun run check
```

Real Figma API tests are opt-in:

```bash
FIGMA_TOKEN=<token> bun run test:integration
```

## License

MIT
