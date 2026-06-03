# fighorse

> [English](../en/README.md) | [中文](README.md) | [Русский](../ru/README.md)

Figma 数据的瑞士军刀，专门打磨成 AI 最容易消化的形状。

`fighorse` 是一个 Bun-first 的 ClojureScript CLI 和 MCP Server。它不生成代码，而是把 Figma REST API 数据整理成 AI 编程工具和开发者都能稳定消费的上下文：完整公开 REST API、结构树、精简 JSON、截图 URL、设计 token、图片/控件导出、manifest、自发现信息和本地经验。

核心理念是 **CLI 为核，MCP 为壳**。CLI 保持白盒、可脚本化、可调试；MCP 让 Cursor、Codex、Kimi、Claude、opencode 等 AI 工具直接调用同一套能力。

## 快速开始

默认路径是仅 CLI 模式。不会启动常驻 MCP 服务，也不会绑定任何端口。

```bash
bun install
bun run install:local
```

```bash
fighorse auth login --token <FIGMA_TOKEN>
fighorse quickstart
```

在 Figma 中，复制你想查看的确切 frame、component 或 group 的链接。除非你在探索，否则不要从整个 page/canvas 开始。

```bash
fighorse quickstart "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
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

为 AI 客户端启用可选的 MCP 服务模式：

```bash
fighorse install --default --mode service --clients cursor,codex,kimi --apply
```

打包可分发的二进制文件。默认包是多平台捆绑包，其 `fighorse` 启动器会自动检测 macOS Intel、macOS Apple Silicon 和 Linux x64/arm64：

```bash
bun run package
bun run package:macos
bun run package:linux
bun run package:darwin-universal
```

## 文档

- [快速开始](quickstart.md)：第一次成功运行 CLI、frame 链接、设计包、可选的 MCP 设置。
- [用户指南](user-guide.md)：安装、认证、CLI、MCP 服务、本地资产导出、经验存储、故障排除。
- [AI 客户端指南](ai-client-guide.md)：AI 工具应如何自发现、调用 MCP/CLI、导出资产、询问平台/资产格式，并记录可复用的经验。
- [设计](design.md)：架构、产品目标、生态系统权衡、自发现/自学习模型、安全边界。

## 核心命令

| 领域 | 命令 |
|------|----------|
| 发现 | `discover`, `doctor`, `smoke`, `url parse`, `mcp config` |
| 官方 REST | `figma-api coverage`, `figma api <operationId>` |
| 设计包 | `design package`, `visual audit`, `project playbook`, `experience summary`, `experience add` |
| Figma 数据 | `file get`, `file nodes`, `node get`, `file tree`, `file compact` |
| 资产 | `image export`, `component export`, `asset download`, `images render`, `images fills` |
| 设计系统 | `components`, `component-sets`, `styles`, `variables`, `tokens extract` |
| 安装 | `install`, `install self`, `install home`, `install auth`, `install binary`, `install client`, `install service`, `install skill`, `install all` |
| MCP | `mcp serve --transport http`, `mcp serve --transport sse --host 127.0.0.1`, 显式 `stdio` 兼容模式 |

## 安全默认值

- Figma 写入操作默认禁用，除非设置 `FIGHORSE_MCP_MODE=write`。
- MCP 本地文件导出需要 `FIGHORSE_MCP_LOCAL_WRITE=allow`。
- 导出路径限制在 `./.fighorse/exports`、`./assets/fighorse` 和 `~/.fighorse/exports`。
- 已安装的 AI 客户端默认使用本地共享 HTTP MCP 端点 `http://127.0.0.1:9449/mcp`；MCP 服务器使用单例锁避免重复的长驻进程。
- 普通 CLI 命令保持一次性进程：不启动 MCP 服务，不绑定端口，不使用 MCP 单例锁。`fighorse install all` 默认仅设置 CLI；只有当你明确需要长驻 MCP 服务时，才使用 `--mode service` 或 `install service --apply`。
- AI 客户端在缺少目标平台和资产格式时必须询问开发者；PNG 只是渲染回退，不是产品决策。

## 开发

```bash
bun run test
bun run build
bun run compile
bun run package
bun run install:local
bun run check
```

真实的 Figma API 测试是可选的：

```bash
FIGMA_TOKEN=<token> bun run test:integration
```

## 协议

[1st Public License (1PL)](https://license.pub/1pl/)
