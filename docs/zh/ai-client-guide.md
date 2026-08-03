# fighorse AI 客户端指南

本指南面向通过 MCP 或 CLI 使用 fighorse 的 AI 编程工具和 agent。契约很简单：先发现，缺少产品假设时询问，带 manifest 导出资产，运行视觉反馈循环，并记录可复用的经验。

## 客户端设置

尽可能使用安装器：

```bash
fighorse install client --client cursor --apply
fighorse install client --client codex --apply
fighorse install client --client kimi --apply
fighorse install client --client claude --apply
fighorse install skill --clients cursor,codex,kimi,claude --apply
```

生成配置但不应用：

```bash
fighorse mcp config --client cursor --transport http
fighorse mcp config --client codex --transport http
fighorse mcp config --client kimi --transport http
fighorse mcp config --client claude --transport http
fighorse mcp config --client opencode --transport http
```

推荐的已安装 MCP 配置：

```json
{
  "mcpServers": {
    "fighorse": {
      "transport": "http",
      "url": "http://127.0.0.1:9449/mcp"
    },
    "figma-official": {
      "transport": "http",
      "url": "https://mcp.figma.com/mcp"
    }
  }
}
```

**为什么两者都要？** fighorse 处理设计到代码的读取工作流（设计包、资产导出、视觉审计、经验学习）和现代 Code Connect 模板工作流。官方 Figma Remote MCP 处理公共 REST 未暴露的产品专属能力：原生 canvas 写入、Code to Canvas、Code Connect 自动映射、FigJam 生成和 Make 资源。它们互补，可以在同一客户端中共存。

- 官方 Remote MCP：`https://mcp.figma.com/mcp` — OAuth 认证，beta 期间免费，未来将按使用量付费。
- 席位要求：共享文件的写入需要 Full seat；Dev seat 在草稿之外是只读的。

推荐的本地服务：

```bash
fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply
fighorse install verify
```

通过 official Rust `rmcp` 2.2 Streamable HTTP 连接 `http://127.0.0.1:9449/mcp`。服务维护独立的 stateful session，校验 Host/Origin，协商 JSON 或 event-stream response，并 graceful shutdown。legacy `/sse`、`/messages` 不存在；`--transport sse` 会失败并引导 HTTP。`/mcp` 返回 `text/event-stream` 是标准协议响应，不是 legacy transport。

安装器先激活 service，再等待 `/health` 并完成 `initialize`/`tools/list`，之后才写 clients 与 skills。manifest、backup 以及 `desired_absent` 删除项位于 `~/.fighorse/install/manifest.json` 和 `~/.fighorse/install/backups/`；`install rollback` 恢复未被用户修改的托管文件及先前服务状态。

Canonical 三目标是 `~/.agents/skills/fighorse/SKILL.md`（Cursor/Kimi/Codex）、`~/.claude/skills/fighorse/SKILL.md`（Claude）、`~/.cursor/rules/fighorse.mdc`（Cursor）。

## 必需的启动流程

连接到 fighorse 后，在实现之前执行以下操作：

1. 调用 `discover_fighorse`。
2. 调用 `doctor` 或读取 `discover_fighorse.production_defaults`。
3. 如有需要，使用 `parse_figma_url` 解析用户提供的 Figma URL。
4. 如果缺少目标平台或资产格式，询问开发者。
5. 针对相关平台、资产格式、文件 key 和节点类型调用 `list_experiences`。
6. 使用 Figma URL、平台和资产格式调用 `get_design_package`。
7. 如果需要本地资产，使用 `manifest: true` 导出所需的图片/component/fill。
8. 在目标代码库中实现 UI。
9. 运行应用，捕获截图，与 Figma 参考对比，调用 `visual_audit` 获取结构化不匹配指导。
10. 调用 `record_experience` 记录在调试过程中发现的可复用经验。

不要跳过自发现。manifest 是 API 契约的一部分，可能比手写客户端指令演进得更快。

## 先询问再猜测

当缺少以下任何一项时，询问开发者：

- 目标平台：web、Android Compose、iOS SwiftUI/UIKit、React Native、Flutter、桌面等。
- 资产格式：png、svg、pdf、jpg、webp 或平台特定的矢量格式。
- 范围：确切的 screen/frame 与广泛的 CANVAS/用户流程节点。
- 当 `./assets/fighorse` 不合适时，生产资产的目标位置。

PNG 只是最安全的 Figma 节点渲染回退。它不是默认的产品决策。

## 推荐的 MCP 工具

优先使用这些高级工具：

- `discover_fighorse`：能力、契约、安全默认值、推荐工作流。
- `doctor`：运行时/认证/本地写入状态。
- `parse_figma_url`：规范化文件 key 和节点 id。
- `get_design_package`：结构化的实现包。
- `list_experiences`：可复用的本地经验。
- `record_experience`：写回可复用的经验。
- `visual_audit`：结构化截图对比、不匹配分析和经验建议。
- `get_project_playbook`：从指导和本地经验组装项目级实现规则。

用于资产：

- `export_images`：渲染节点截图/切片。
- `export_component`：将 component/control 节点导出为 png/svg/pdf/jpg。
- `download_image_fills`：下载设计引用的图片 fill。

仅当设计包不足时才使用底层 Figma 工具：

- `get_file_compact`
- `get_node`
- `get_file_tree`
- `get_image`
- `get_image_fills`
- `get_file_tokens`

需要精确的 OpenAPI 对等性时使用生成的官方 REST 工具。这些工具命名为 `figma_<operation_id_in_snake_case>`，例如 `figma_get_file`、`figma_get_developer_logs`、`figma_put_webhook` 和 `figma_post_variables`。在只读 MCP 模式下，生成的 Figma 写入工具被隐藏和阻止；只有当开发者明确允许 Figma 修改时，才设置 `FIGHORSE_MCP_MODE=write`。

只有当用户要把代码组件连接到 Figma Dev Mode 时才使用 Code Connect 工具：

- `parse_code_connect_template`：检查已提供的 Code Connect documents。
- `validate_code_connect`：确认目标 Figma 节点是组件或组件集。
- `preview_code_connect`：把模板代码发给 Figma 做真实片段渲染；需要 `FIGHORSE_MCP_CODE_CONNECT=allow`。
- `publish_code_connect`：发布映射；需要 `FIGHORSE_MCP_CODE_CONNECT=allow` 和 `FIGHORSE_MCP_MODE=write`。
- `unpublish_code_connect`：删除精确的 node+label 映射；需要同样两个开关。

生成模板时，AI 客户端应使用自己的文件工具读取目标代码仓库，再把明确的组件上下文交给 CLI `fighorse code-connect generate`。fighorse 不通过 MCP 扫描或执行用户代码仓库。

支持 MCP 资源和 prompt 的客户端还可以读取：

- `fighorse://capabilities`
- `fighorse://coverage`
- `fighorse://workflow/design-replication`
- `fighorse://experience/summary`
- Prompt: `fighorse_design_replication`
- Prompt: `fighorse_api_coverage`

## CLI 等效命令

如果 MCP 不可用，运行等效的 CLI 命令：

```bash
fighorse discover --format json
fighorse doctor --format json
fighorse url parse "<figma-url>"
fighorse experience summary --platform <platform> --asset-format <asset-format> --format json
fighorse design package "<figma-url>" --platform <platform> --asset-format <asset-format> --output ./.fighorse/exports/package.json
fighorse image export <file_key> --ids <node_ids> --dir ./.fighorse/exports --manifest
fighorse component export <file_key> --ids <node_ids> --format <asset-format> --dir ./assets/fighorse --manifest
fighorse asset download <file_key> --dir ./assets/fighorse --manifest
fighorse visual audit "<figma-url>" --screenshot <app-screenshot-path> --platform <platform> --asset-format <asset-format>
fighorse project playbook --platform <platform> --asset-format <asset-format>
fighorse figma-api coverage --format json
fighorse figma api getFile --params '{"file_key":"<file_key>","depth":1}'
fighorse code-connect generate "<figma-component-url>" --context code-context.json
fighorse code-connect parse --dir .
fighorse code-connect preview --documents docs.json
fighorse code-connect publish --documents docs.json --dry-run
```

## 本地写入策略

MCP Figma 写入模式和本地文件系统写入模式是独立的。

- `FIGHORSE_MCP_MODE=readonly`：默认；不暴露 Figma 写入工具。
- `FIGHORSE_MCP_MODE=write`：暴露已实现的 Figma 写入工具。
- `FIGHORSE_MCP_LOCAL_WRITE=deny`：默认；阻止本地导出工具。
- `FIGHORSE_MCP_LOCAL_WRITE=allow`：在批准的根目录内允许本地导出工具。
- `FIGHORSE_MCP_CODE_CONNECT=deny`：默认；MCP 不能把 Code Connect 模板代码发送给 Figma。
- `FIGHORSE_MCP_CODE_CONNECT=allow`：允许 Code Connect preview/publish 把模板代码发给 Figma。

批准的根目录：

- `./.fighorse/exports`
- `./assets/fighorse`
- `~/.fighorse/exports`

导出工具始终请求 `manifest: true`。读取 manifest 来定位文件，而不是推断文件名。

## 设计包契约

将 `get_design_package` 视为实现的真理来源。重要字段：

- `implementation_target`：平台和资产格式假设及警告。
- `target`：选定节点的身份、类型、尺寸，以及是否可能太宽泛。
- `screen_candidates` 和 `component_candidates`：可能需要检查、缩小或导出的 frame/component。
- `context`：用于实现的精简设计数据。
- `tokens`：提取的颜色、排版、间距和效果。
- `token_confidence` 和 `missing_font_diagnostics`：token/字体可靠性的质量信号。
- `screenshots`：Figma 返回的渲染参考。
- `asset_export_plan`：精确的下一步导出命令和 MCP 调用。
- `learned_experience`：之前运行学到的经验。
- `implementation_risk_checklist`：在最终确定前需要验证的具体风险。
- `diagnostics`：就绪状态和警告。

如果 `diagnostics.status` 不是 `ready`，尽可能在编码前按照警告操作。CANVAS 或用户流程目标通常应缩小到具体的移动/web frame。

## 视觉保真循环

首次代码生成后，实现并未完成。使用此循环：

1. 从 fighorse 导出参考截图或切片。
2. 使用包中的精确尺寸、排版、间距和颜色实现目标屏幕。
3. 在目标平台中运行应用。
4. 在预期的视口/设备尺寸捕获截图。
5. 将应用截图与 Figma 参考对比。
6. 修复布局、排版、资产、裁剪、滚动、状态栏和重叠问题。
7. 重复直到差异被理解并可接受。

真实使用中的已知经验：

- 重复的 sibling 应映射到平台列表/线性容器，而非通用堆叠容器。
- 紧凑的卡片通常需要自己的字体大小和行高；不要盲目复用全卡片排版。
- 移动屏幕需要滚动安全的布局决策，而非固定的垂直堆叠。
- 除非显式全屏或处理安全区域，否则真实设备系统 UI 可能与 Figma 状态栏重叠。
- 缺失的字体应被诊断并有意处理。

## 记录经验

记录将帮助未来不相关任务的经验。好的经验是平台感知但非项目特定的：

```json
{
  "category": "layout",
  "platform": "android-compose",
  "asset_format": "png",
  "summary": "Repeated list items overlapped",
  "lesson": "Use a LazyColumn or Column for repeated sibling rows; use Box only for intentional overlays.",
  "tags": ["list", "overlap", "compose"]
}
```

不要记录：

- 密钥、令牌、私有 URL 或本地绝对路径。
- 不可复用的一次性项目决策。
- 仅仅是重述设计内容的经验。

## 客户端特定设置

所有客户端应接收相同的 fighorse 契约。差异应仅限于配置文件形状和传输。推荐的公共设置是在 `http://127.0.0.1:9449/mcp` 的一个共享本地 HTTP MCP 服务。

### Cursor

安装：

```bash
fighorse install --default --mode service --clients cursor --apply
```

预期的配置形状：

```json
{
  "mcpServers": {
    "fighorse": {
      "url": "http://127.0.0.1:9449/mcp"
    }
  }
}
```

验证：

```bash
fighorse quickstart --format json
fighorse doctor --format json
```

常见故障：Cursor 配置为重复生成 stdio。将该配置替换为共享的 HTTP 端点，除非客户端无法连接本地 HTTP。

### Codex

安装：

```bash
fighorse install --default --mode service --clients codex --apply
```

预期的生成 TOML：

```toml
[mcp_servers.fighorse]
url = "http://127.0.0.1:9449/mcp"
enabled = true
startup_timeout_sec = 60
```

验证：

```bash
fighorse install status
curl http://127.0.0.1:9449/health
```

常见故障：Codex 每次启动时都可能初始化新的 Streamable HTTP session。`/mcp` 必须创建独立的 stateful session，并返回标准 JSON 或 event-stream response。

### Kimi

安装：

```bash
fighorse install --default --mode service --clients kimi --apply
```

预期的命令形状：

```bash
kimi mcp add --transport http fighorse http://127.0.0.1:9449/mcp
```

预期 payload：`{"transport":"http","url":"http://127.0.0.1:9449/mcp"}`。

验证：

```bash
fighorse quickstart --format json
```

常见故障：旧版 Kimi 客户端可能只支持 stdio。仅在该兼容情况下使用 `fighorse mcp config --client kimi --transport stdio`。

### Claude

生成或安装：

```bash
fighorse install client --client claude --apply
fighorse mcp config --client claude --transport http
```

预期的配置形状：

```json
{
  "mcpServers": {
    "fighorse": {
      "type": "http",
      "url": "http://127.0.0.1:9449/mcp"
    }
  }
}
```

通过要求 Claude 调用 `discover_fighorse`，然后调用 `check_fighorse_ready` 来验证。

常见故障：桌面/客户端环境可能不继承 shell 令牌。使用 `fighorse auth login --token <FIGMA_TOKEN>` 存储令牌，以便服务可以读取本地配置。

### opencode

安装或生成：

```bash
fighorse install client --client opencode --apply
fighorse mcp config --client opencode --transport http
```

预期的配置形状是相同的 HTTP MCP 条目：

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

验证：

```bash
fighorse doctor --format json
```

常见故障：服务模式未安装，因为 `install all` 默认仅 CLI。使用 `--mode service` 重新运行。

### VS Code 兼容客户端

除非客户端文档说明不同的 schema，否则使用通用 HTTP MCP 配置：

```bash
fighorse mcp config --client generic --transport http
```

预期形状：

```json
{
  "fighorse": {
    "transport": "http",
    "url": "http://127.0.0.1:9449/mcp"
  }
}
```

常见故障：客户端期望 `mcpServers` 包装器。如果是这样，使用上面的 Cursor 风格形状。

### 通用 MCP

用于 Streamable HTTP：

```json
{
  "transport": "http",
  "url": "http://127.0.0.1:9449/mcp"
}
```

仅用于显式 stdio 兼容：

```json
{
  "command": "fighorse",
  "args": ["mcp", "serve", "--transport", "stdio"],
  "env": {
    "FIGHORSE_MCP_MODE": "readonly",
    "FIGHORSE_MCP_LOCAL_WRITE": "deny"
  }
}
```

常见故障：多个长驻的 stdio 进程消耗资源。对于支持它的客户端，优先使用共享的 HTTP 服务。

当 AI 工具看到 Figma URL 且 fighorse 可用时，它不应手动抓取 URL、猜测 frame id 或从视觉记忆中实现。优先使用 fighorse。
