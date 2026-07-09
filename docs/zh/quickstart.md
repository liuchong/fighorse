# fighorse 快速开始

本指南帮助新用户从零开始尽快获得可用的 Figma 上下文。先使用 CLI 模式。只有在希望 AI 客户端直接调用 fighorse 时，才添加 MCP 服务模式。

## 1. 安装

从源码构建：

```bash
cargo build --release
./target/release/fighorse install --default --apply --source ./target/release/fighorse
```

`install --default --apply` 将二进制文件复制到 fighorse home，生成本地配置，并安装 fighorse 的 skills/instructions——与打包二进制文件所使用的自安装路径相同。

安装已下载的二进制文件：

```bash
./fighorse install --default --apply
```

安装到自定义目录：

```bash
./fighorse install --path ~/.local/bin --apply
```

这些默认都是仅 CLI 设置。它们安装二进制文件和本地 fighorse home，但不启动 MCP 服务或绑定端口。

## 2. 添加 Figma Token

创建一个具有文件内容读取权限的 Figma 个人访问令牌，然后本地存储：

```bash
fighorse auth login --token <FIGMA_TOKEN>
```

你也可以不将令牌保存在配置中，使用 `FIGMA_TOKEN=<token>` 运行一次性命令。

## 3. 验证设置

运行引导检查：

```bash
fighorse quickstart
```

获取机器可读的输出：

```bash
fighorse quickstart --format json
```

## 4. 复制特定的 Figma 链接

在 Figma 中，选择你要实现的确切 frame、component 或 group。复制该选区的链接。除非你在探索，否则不要从整个页面或广泛的 canvas 开始。

验证链接：

```bash
fighorse quickstart "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
```

## 5. 获取设计包

先询问目标平台和资产格式。然后构建包：

```bash
fighorse design package "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>" \
  --platform web-react \
  --asset-format svg \
  --output ./.fighorse/exports/package.json
```

这是 AI 实现的主要上下文来源。它包含精简的结构、截图、token、诊断、资产导出建议和本地学习到的经验。

## 6. 可选：使用 MCP 服务模式

只有当 AI 客户端应该直接调用 fighorse 时才使用服务模式：

```bash
fighorse install --default --mode service --clients cursor,codex,kimi --apply
```

已安装的客户端应使用：

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

该服务默认仅限本地主机，受单例锁保护，并使用支持重复客户端握手的 Streamable HTTP 端点。

## 7. 如何向你的 AI Agent 提问

MCP 连接后，粘贴一个具体的 Figma frame 链接并询问：

```text
使用 fighorse 检查这个 Figma frame。先调用 discover_fighorse，然后调用 list_experiences，再调用 get_design_package。如果缺少平台或资产格式，请问我。导出资产时带 manifest，不要猜测。
```

## 故障排除

- 缺少令牌：运行 `fighorse auth login --token <FIGMA_TOKEN>`。
- 链接太宽泛：复制选定 frame 或 component 的链接。
- MCP 服务未运行：使用 `fighorse install --default --mode service --clients cursor,codex,kimi --apply`。
- Codex 报告意外的内容类型：验证 `curl http://127.0.0.1:9449/health`；`/mcp` 端点必须返回 MCP JSON/SSE，而不是 `text/plain`。
- 本地导出被拒绝：使用 `./.fighorse/exports`、`./assets/fighorse` 或 `~/.fighorse/exports`。
