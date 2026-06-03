# fighorse 用户指南

`fighorse` 是一个公共优先、开源的 Figma CLI + MCP。它将公共 Figma REST API 数据转化为 AI 友好的上下文和开发者友好的 CLI 输出。第一次运行应该很简单：安装、添加令牌、粘贴一个具体的 frame 链接、生成一个有用的设计包。第二次运行可以更深入：资产 manifest、视觉审计、项目 playbook、完整的 REST 覆盖和本地经验学习。

当你需要可复现的命令、脚本、CI 或快速检查时，使用 CLI。当 AI 编程工具应该直接调用 fighorse 时，使用 MCP 服务。

## 从源码安装

最快的路径见[快速开始](quickstart.md)。默认安装模式是仅 CLI，不启动 MCP 服务进程。

```bash
bun install
bun run install:local
./dist/fighorse --help
```

安装编译后的二进制文件和可选的 AI 客户端集成：

```bash
./dist/fighorse install status
./dist/fighorse install auth --apply
./dist/fighorse install --default --apply
```

仅在需要时安装 MCP 服务和 AI 客户端集成：

```bash
./dist/fighorse install client --client cursor --apply
./dist/fighorse install client --client codex --apply
./dist/fighorse install client --client kimi --apply
./dist/fighorse install service --service launchd --apply
./dist/fighorse install --default --mode service --clients cursor,codex,kimi --apply
```

安装命令默认生成可审查的文件。只有当你希望 fighorse 修改用户级客户端配置、skill/rule 位置、二进制链接或服务管理器时，才添加 `--apply`。

## 打包二进制文件

默认的打包命令构建多平台捆绑包。其顶层 `fighorse` 启动器检测 `uname -s` 和 `uname -m`，然后运行匹配的原生二进制文件，支持 macOS Intel、macOS Apple Silicon、Linux x64 或 Linux arm64：

```bash
bun run package
tar -tzf dist/fighorse-multi-platform.tar.gz
```

你也可以构建按平台划分的归档：

```bash
bun run package:macos
bun run package:darwin-x64
bun run package:darwin-arm64
bun run package:linux-x64
bun run package:linux-arm64
bun run package:all
```

`package:macos` 创建两个 macOS 架构归档加上一个自动检测 Intel 与 Apple Silicon 的 macOS 捆绑包。如果你特别需要一个包含两个 macOS slice 的单个 Mach-O 文件，在 macOS 上运行 `bun run package:darwin-universal`，需要 `lipo` 可用。

## 认证

从 Figma 开发者设置创建一个 Figma 个人访问令牌，然后本地存储：

```bash
fighorse auth login --token <FIGMA_TOKEN>
```

你也可以使用环境变量运行一次性命令：

```bash
FIGMA_TOKEN=<FIGMA_TOKEN> fighorse file tree <file_key>
```

`install auth --apply` 读取 `--token`、stdin、`FIGMA_TOKEN` 或 `FIGMA_API_KEY`，并将令牌存储在 `~/.fighorse/config.json` 中。命令输出会遮盖令牌。

## 验证就绪状态

```bash
fighorse quickstart
fighorse quickstart "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
fighorse doctor --format json
fighorse discover --format json
fighorse smoke "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
fighorse figma-api coverage --format json
```

`smoke` 使用真实的 Figma 访问并返回 `fighorse.smoke.v1`。`ok: true` 表示正常的设计包路径已就绪。`ok: false` 且 `diagnostics.status: partial` 仍可能意味着访问成功；按照警告操作，通常需要指定平台、资产格式或确切的 frame 节点。

`figma-api coverage` 报告与 vendored Figma REST OpenAPI 快照的对比情况。当前注册表跟踪 48 个公共操作，并通过 API 层、通用 CLI 操作调度和生成的官方 MCP 工具暴露每个操作。

## 官方 REST API 覆盖

对于常规设计工作使用产品命令，当你需要精确的底层 API 对等性时使用通用 REST 调度：

```bash
fighorse figma-api coverage --format md
fighorse figma api getFile --params '{"file_key":"<file_key>","depth":1}'
fighorse figma api putWebhook --params '{"webhook_id":"<id>"}' --body '{"status":"PAUSED"}' --yes
```

`figma api` 接受来自 OpenAPI 注册表的官方 `operationId` 名称。读取操作正常运行。写入操作需要 `--yes`，因为它们可能修改评论、变量、webhook 或开发资源。

## 检查设计

解析粘贴的 Figma URL：

```bash
fighorse url parse "https://www.figma.com/design/<fileKey>/<name>?node-id=1-2"
```

查看结构：

```bash
fighorse file tree <file_key> --depth 2
fighorse node get <file_key> <node_id> --depth 3
```

获取精简的 AI 上下文：

```bash
fighorse file compact <file_key> --depth 2 --max-tokens 8000
fighorse file get <file_key> --depth 2 | fighorse compact --max-tokens 8000
```

提取设计 token：

```bash
fighorse file tokens <file_key> --format json
fighorse tokens extract <file_key> --format css --output ./tokens.css
```

## 构建设计包

对于 AI 实现工作，优先使用 `design package` 而非底层调用：

```bash
fighorse design package "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>" \
  --platform android-compose \
  --asset-format png \
  --max-tokens 8000 \
  --output ./.fighorse/exports/package.json
```

该包包含：

- `source`：解析后的文件 key 和节点 id。
- `file` 和 `target`：元数据和选定节点摘要。
- `implementation_target`：平台和资产格式假设。
- `screen_candidates` 和 `component_candidates`：可能需要检查或导出的 frame/component。
- `fidelity_workflow`：视觉验证步骤。
- `asset_export_plan`：建议的 CLI 和 MCP 资产导出调用。
- `learned_experience`：之前运行学到的本地经验。
- `token_confidence`、`missing_font_diagnostics` 和 `implementation_risk_checklist`：编码前的 AI 就绪检查。
- `context`、`tokens`、`screenshots` 和可选的 `assets`。
- `diagnostics`：缺少平台、缺少资产格式、CANVAS 目标、截断、缺少截图或缺少 token 的警告。

如果平台或资产格式未知，请在实现前询问开发者。PNG 是渲染回退，不是自动的产品决策。

## 导出资产

当实现需要真实图像文件而非临时渲染 URL 时，使用本地导出：

```bash
fighorse image export <file_key> --ids <node_ids> --format png --dir ./.fighorse/exports --manifest
fighorse component export <file_key> --ids <component_node_ids> --format svg --dir ./assets/fighorse --manifest
fighorse asset download <file_key> --dir ./assets/fighorse --manifest
```

推荐的输出位置：

- `./.fighorse/exports`：临时切片、截图、manifest 和调试资产。
- `./assets/fighorse`：打算被应用代码引用或打包的资产。
- `~/.fighorse/exports`：跨项目的临时导出。

导出命令写入安全的文件名，并可以创建 `manifest.json`。使用 manifest 而非解析终端输出。

## MCP 服务器

对于已安装的客户端，优先使用共享的本地 HTTP 服务，这样 Cursor、Codex、Kimi 和其他客户端可以复用一个 `fighorse` 进程，而不是各自生成 stdio 子进程：

```json
{
  "mcpServers": {
    "fighorse": {
      "transport": "http",
      "url": "http://127.0.0.1:9449/mcp"
    }
  }
}
```

尽可能通过显式的服务路径安装和启动本地服务：

```bash
fighorse install --default --mode service --clients cursor,codex,kimi --apply
```

对于开发，你也可以直接运行 `fighorse mcp serve --transport sse --host 127.0.0.1 --port 9449`。

HTTP 端点：

```text
http://127.0.0.1:9449/mcp
http://127.0.0.1:9449/sse
http://127.0.0.1:9449/manifest
http://127.0.0.1:9449/health
```

该服务默认绑定到 `127.0.0.1`，并在 `~/.fighorse/runtime` 中使用单例锁。只有当你打算将服务暴露到本地主机之外时，才显式使用 `--host`。只有在客户端无法连接本地 HTTP 端点时才使用 stdio。

维护者实现说明：Codex 等 Streamable HTTP 客户端每次启动时都可能初始化一个新的 MCP 连接。因此 `/mcp` 端点必须容忍重复的握手。在无状态模式下，每个请求创建和关闭一个新的传输/服务器。复用一个已经初始化的 `StreamableHTTPServerTransport` 可能导致第一次握手通过，后续握手失败，返回 `500 text/plain` 而非 MCP JSON/SSE 响应。

正常的 CLI 命令如 `fighorse file get`、`fighorse design package` 和 `fighorse image export` 都是一次性进程。它们每次允许启动，不启动 MCP 服务，不绑定端口，不占用 MCP 单例锁，输出写入后应退出。Figma HTTP 调用和图片下载使用默认 `120000` 毫秒的 `FIGHORSE_HTTP_TIMEOUT_MS`，`SIGINT`/`SIGTERM` 在退出前中止正在进行的请求。`fighorse install --default --apply` 默认仅设置 CLI；只有当你明确希望 fighorse 配置或启动长驻 MCP 服务时，才使用 `fighorse install --default --mode service --apply` 或 `fighorse install service --apply`。

## 安全模式

Figma 写入工具默认隐藏，除非启用：

```bash
FIGHORSE_MCP_MODE=write fighorse mcp serve --transport sse
```

本地文件导出单独控制：

```bash
FIGHORSE_MCP_LOCAL_WRITE=allow fighorse mcp serve --transport sse
```

即使启用了本地写入，导出路径也会经过验证，必须保持在 `./.fighorse/exports`、`./assets/fighorse` 或 `~/.fighorse/exports` 之下。

## 经验存储

fighorse 将可复用的经验存储为追加式 JSONL。这使得未来的 AI 运行可以从之前的视觉调试中学习，而无需更改 fighorse 二进制文件。

默认路径：

- Home：`~/.fighorse`。
- 全局经验：`~/.fighorse/experience/global.jsonl`。
- 项目经验：`./.fighorse/experience.jsonl`（在 `fighorse install project` 之后）。
- 精确覆盖：`FIGHORSE_EXPERIENCE_PATH`。
- 范围覆盖：`FIGHORSE_EXPERIENCE_SCOPE=global|project|merged`。

命令：

```bash
fighorse install project
fighorse experience schema
fighorse experience summary --platform android-compose --asset-format png --format md
fighorse experience add \
  --summary "Repeated list items overlapped" \
  --lesson "Use a list or linear container for repeated siblings; use stacking containers only for intentional overlays." \
  --category layout \
  --platform android-compose \
  --asset-format png
```

## 常见工作流

在实现 Figma 屏幕之前使用 fighorse：

```bash
fighorse discover --format json
fighorse experience summary --platform web-react --asset-format svg
fighorse design package "<figma-url>" --platform web-react --asset-format svg --output ./.fighorse/exports/package.json
fighorse visual audit "<figma-url>" --screenshot ./.fighorse/exports/app-screen.png --platform web-react --asset-format svg
fighorse project playbook --platform web-react --asset-format svg
```

同步 token：

```bash
fighorse file tokens <design_system_key> --format css --output src/styles/tokens.css
```

批量导出 frame：

```bash
IDS=$(fighorse file get <file_key> --depth 2 | jq -r '.. | objects | select(.type == "FRAME") | .id' | paste -sd, -)
fighorse image export <file_key> --ids "$IDS" --dir ./.fighorse/exports --manifest
```

## 故障排除

- 第一次运行不清楚：运行 `fighorse quickstart "<figma-frame-url>"` 并按照 `next_steps` 操作。
- `doctor.auth.has_token` 为 false：运行 `fighorse auth login --token <FIGMA_TOKEN>` 或 `fighorse install auth --apply`。
- `doctor.checks` 报告 MCP 服务未运行：对于仅 CLI 工作可以忽略，或者为 AI 客户端运行 `fighorse install --default --mode service --clients cursor,codex,kimi --apply`。
- Codex/Cursor 报告 `text/plain` 或重复的 initialize 失败：升级后重启 fighorse 服务；`/mcp` 必须支持重复的 Streamable HTTP 握手。
- `smoke.ok` 为 false 但文件元数据存在：按照 `diagnostics.warnings` 操作；通常选定的目标太宽泛或缺少平台/资产格式。
- MCP 导出工具报告本地写入禁用：在 MCP 服务器环境中设置 `FIGHORSE_MCP_LOCAL_WRITE=allow`。
- 导出路径被拒绝：使用 `./.fighorse/exports`、`./assets/fighorse` 或 `~/.fighorse/exports`。
- AI 实现了整个用户流程页面：在实现前将 Figma URL 缩小到具体的 Frame/Screen 节点。
