# fighorse

> [English](../en/README.md) | [中文](README.md) | [Русский](../ru/README.md)

Figma 数据的瑞士军刀，专门打磨成 AI 最容易消化的形状。

`fighorse` 是一个 Rust CLI 和 MCP 服务器。它不生成代码，而是把 Figma REST API 数据整理成 AI 编程工具和开发者都能稳定消费的上下文：完整公开 REST API、结构树、精简 JSON、截图 URL、设计 token、图片/控件导出、manifest、自发现信息和本地经验。

核心理念是 **CLI 为核，MCP 为壳**。CLI 保持白盒、可脚本化、可调试；MCP 让 Cursor、Codex、Kimi、Claude、opencode 等 AI 工具直接调用同一套能力。

## 快速开始

默认路径是仅 CLI 模式。不会启动常驻 MCP 服务，也不会绑定任何端口。

```bash
cargo build --release
./target/release/fighorse install --default --apply --source ./target/release/fighorse
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
fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply
fighorse install verify
# 仅安装 Claude：
fighorse install client --client claude --apply
```

服务安装是事务性的：先写入二进制和服务文件，等待 `/health`，在 `/mcp` 完成真实的 `initialize` 与 `tools/list` 握手，之后才写客户端配置与 skill。`~/.fighorse/install/manifest.json` 记录托管文件和 `desired_absent` 删除项，备份及迁移冲突保存在 `~/.fighorse/install/backups/`；`fighorse install rollback` 只恢复仍与 manifest 一致的托管文件及先前服务状态。

四种原生 HTTP payload 分别是：Cursor `{"url":"http://127.0.0.1:9449/mcp"}`、Kimi `{"transport":"http","url":"http://127.0.0.1:9449/mcp"}`、Claude `{"type":"http","url":"http://127.0.0.1:9449/mcp"}`，Codex 使用 `[mcp_servers.fighorse]` 和同一 URL。

三处 canonical 目标是：Cursor/Kimi/Codex 共用 `~/.agents/skills/fighorse/SKILL.md`，Claude 使用 `~/.claude/skills/fighorse/SKILL.md`，Cursor 另用 `~/.cursor/rules/fighorse.mdc`。

使用 Cargo 打包可分发的二进制文件。使用匹配的 Rust 工具链按目标平台交叉编译
（Linux 目标可使用 `cargo-zigbuild`）：

```bash
cargo build --release
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
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
| Code Connect | `code-connect generate`, `code-connect parse`, `code-connect validate`, `code-connect preview`, `code-connect publish`, `code-connect unpublish` |
| 设计包 | `design package`, `visual audit`, `project playbook`, `experience summary`, `experience add` |
| Figma 数据 | `file get`, `file nodes`, `node get`, `file tree`, `file compact` |
| 资产 | `image export`, `component export`, `asset download`, `images render`, `images fills` |
| 设计系统 | `components`, `component-sets`, `styles`, `variables`, `tokens extract` |
| 安装 | `install`, `install self`, `install home`, `install auth`, `install binary`, `install client`, `install service`, `install skill`, `install all`, `install verify`, `install rollback` |
| MCP | `mcp serve --transport http`, 显式 `stdio` 兼容模式 |

## 安全默认值

- Figma 写入操作默认禁用，除非设置 `FIGHORSE_MCP_MODE=write`。
- MCP 本地文件导出需要 `FIGHORSE_MCP_LOCAL_WRITE=allow`。
- MCP Code Connect 预览/发布需要 `FIGHORSE_MCP_CODE_CONNECT=allow`；发布/删除还需要 `FIGHORSE_MCP_MODE=write`。
- 导出路径限制在 `./.fighorse/exports`、`./assets/fighorse` 和 `~/.fighorse/exports`。
- 已安装的 AI 客户端默认使用本地共享 HTTP MCP 端点 `http://127.0.0.1:9449/mcp`；MCP 服务器使用单例锁避免重复的长驻进程。
- `/mcp` 是 official Rust `rmcp` 2.2 Streamable HTTP 服务，提供相互独立的 stateful session、Host/Origin 校验、JSON 或 event-stream response 以及 graceful shutdown。legacy `/sse` 和 `/messages` 不再提供；`--transport sse` 会失败并引导迁移到 `--transport http`。
- 新安装的服务和显式 stdio 配置默认 `FIGHORSE_MCP_LOCAL_WRITE=deny`；迁移时只保留既有的显式 `allow`。
- 普通 CLI 命令保持一次性进程：不启动 MCP 服务，不绑定端口，不使用 MCP 单例锁。`fighorse install all` 默认仅设置 CLI；只有当你明确需要长驻 MCP 服务时，才使用 `--mode service` 或 `install service --apply`。
- AI 客户端在缺少目标平台和资产格式时必须询问开发者；PNG 只是渲染回退，不是产品决策。

## Code Connect

fighorse 原生支持现代无本地执行模板的 Code Connect 文件（`.figma.ts`、`.figma.js` 和 `.figma.batch.json`），不需要 Node.js，也不包装官方 Code Connect CLI。它可以根据 AI 明确提供的代码组件上下文生成模板，本地解析模板但不执行代码，校验 Figma 组件节点，并通过 Figma 的真实远端预览、发布和删除协议让 Dev Mode 生效。

```bash
fighorse code-connect generate "<figma-component-url>" --context code-context.json
fighorse code-connect parse --dir .
fighorse code-connect preview --documents docs.json
fighorse code-connect publish --documents docs.json --dry-run
fighorse code-connect publish --documents docs.json --yes --force
fighorse code-connect unpublish --node "<figma-component-url>" --label React --dry-run
```

自动 Code Connect 映射发现仍是 Figma 产品能力；需要 Figma 产品内自动映射时，使用官方 Figma Remote MCP。

## 开发

```bash
cargo test
cargo build --release
cargo clippy
```

真实的 Figma API 测试是可选的：

```bash
FIGMA_INTEGRATION_TESTS=1 FIGMA_TOKEN=<token> cargo test -- --ignored
```

## 协议

[1st Public License (1PL)](https://license.pub/1pl/)（全文见仓库根目录 [LICENSE](../../LICENSE) 文件）
