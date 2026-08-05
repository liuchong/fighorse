# fighorse 设计

`fighorse` 是一个 Rust CLI 和 MCP 服务器，用于将 Figma REST API 数据转化为人类和 AI agent 的实现级上下文。它是公共优先的基础设施：对首次用户来说足够简单，可以快速成功获得设计包；对团队和 AI 工具来说足够深入，可以随着时间的推移构建可复现的视觉工作流。

## 产品目标

这个项目存在，因为常见的 Figma 到 AI 的路径各自缺少一些重要的东西：

- 官方 Figma MCP 很强大，但黑盒行为使调试困难，未来的定价或计划边界可能影响可用性。
- 仅截图的 prompt 丢失了精确的版式、颜色、排版、约束和组件元数据。
- 原始 Figma JSON 对 LLM 上下文窗口来说太大太嘈杂。
- 仅 MCP 的社区工具在 IDE 内很有用，但对脚本、CI、可复现调试和非 MCP agent 来说很弱。

fighorse 的目标是提供一个白盒数据管道，具有三个渐进层次：

- 简单首次运行：安装、令牌、特定 frame 链接、快速开始检查、设计包。
- 深入第二次运行：资产 manifest、精确 REST 调度、视觉审计、项目 playbook。
- 长期学习：改进未来 AI 运行的本地经验记录，无需隐藏记忆。

该管道提供：

- 准确的 Figma 事实：节点结构、尺寸、样式、版式、图片、token、元数据。
- AI 就绪上下文：精简、有预算意识、明确假设。
- 视觉参考：截图/渲染 URL 和带 manifest 的本地导出资产。
- 工具中立访问：CLI 优先，MCP 作为适配器，可安装的 skill/rule 和本地 AI 插件资源包用于 AI 客户端。
- 反馈记忆：本地经验存储，使视觉调试经验被复用。

资源目录是产品层聚合器，不是新的爬虫或数据存储。来源解析严格区分团队、
项目、browser-root 和具体设计 URL；CLI 与 MCP 调用同一个异步函数。
它先做不回传身份信息的用户验证探测，保留项目、设计库和文件探测的部分
成功结果，只输出有界的结构化诊断。它不缓存、不批量下载，也不绕过 Figma
scope 和团队访问权。URL 解析会暴露 `catalog_eligible` 和
`browser_root_not_enumerable`，让客户端避免把 `/files/<browser-root>`
误送进目录工具。

## 核心原则：CLI 内核，MCP 外壳

CLI 是主要产品边界。MCP 向 AI 工具暴露相同的能力，但应保持轻量。

```text
Figma REST API
  -> API 模块 + OpenAPI 操作注册表
  -> 产品层：compact/filter/tokens/assets/resource-catalog/design-package/visual-audit/playbook
  -> CLI 输出、文件、manifest
  -> MCP 工具/资源/prompt、AI 客户端、脚本、CI
```

这使系统保持：

- 公共优先：用户可以在学习 MCP 之前通过 CLI 成功。
- 可检查：开发者可以运行 AI 工具调用的相同命令。
- 可脚本化：shell、CI 和自定义 agent 可以直接使用二进制文件。
- 传输中立：共享 Streamable HTTP、显式标准 stdio MCP 和 CLI 共享业务行为。
- 更易于测试：纯转换和 API 包装器与客户端配置保持分离。

## AI 插件资源包

AI 插件资源包用于打包客户端侧指导，不改变运行时权限。
`fighorse install ai-plugin --clients cursor,codex,kimi,claude,opencode,gemini --apply`
会从 `assets/ai-plugin/` 的共享模板生成
`~/.fighorse/ai-plugin/fighorse/`。

资源包包含薄客户端 manifest（`.cursor-plugin/plugin.json`、
`.claude-plugin/plugin.json`、`gemini-extension.json`、`server.json`、
`.mcp.json`）以及共享 workflow skills：`fighorse`、
`fighorse-design-to-code`、`fighorse-canvas-write`、
`fighorse-resource-catalog`、`fighorse-code-connect` 和
`fighorse-self-learning`。

该资源包只用于本地分发，不声明 Verified by Cursor，并继续使用
`http://127.0.0.1:9449/mcp`。它不会打开 Figma 写入、本地文件导出、画布写入、
Plugin API JavaScript 或 Code Connect 发布权限；这些仍是独立的 fighorse gate。

## 分层架构

```text
L4 适配器
  mcp serve、resources/prompts、install client、install service、生成的 skills/rules

L3 生成的上下文
  design package、visual audit、project playbook、markdown、tokens、tree、schema、manifests

L2 转换
  compact、filter、diff、diagnostics、experience matching、API coverage reports

L1 Figma API 和资产
  OpenAPI 注册表、操作调度器、files、nodes、images、comments、components、styles、variables、webhooks、downloads
```

L2 和 L3 是主要差异化因素。fighorse 不仅仅是包装 REST 端点；它将 Figma 数据重塑为 AI agent 可以在不淹没于噪音中使用的形式。

## 完整 REST 覆盖

fighorse 为公共 Figma REST 快照维护一个显式的 OpenAPI 操作注册表。注册表当前跟踪 50 个操作，并被以下使用：

- `src/fighorse/api` 中的 API 包装器。
- 通用 CLI 调度：`fighorse figma api <operationId> --params '{...}'`。
- 生成的官方 MCP 工具：`figma_<operation_id_in_snake_case>`。
- 发现和覆盖报告：`fighorse figma-api coverage`。
- 防止缺失或漂移端点的契约测试。

通用官方层与产品工具分离。底层 REST 工具保留 Figma 语义；`get_design_package`、`visual_audit` 和 `get_project_playbook` 等产品工具在其上添加 AI 工作流指导。

## 设计包

`fighorse design package` 和 MCP `get_design_package` 是设计实现的首选高级接口。该包组合了：

- 解析后的 Figma URL 和选定的目标节点。
- 精简的结构上下文。
- Token 和实现提示。
- Figma 渲染参考和可选的资产 URL。
- 平台和资产格式假设。
- Scope、screen 和 component 候选。
- 带 CLI 示例和 MCP 调用的导出计划。
- 本地学习到的经验。
- Token 置信度、缺失字体诊断和实现风险检查清单。
- 诊断和下一步警告。

该包设计为既可机器读取又易于检查。它不仅应告诉 AI 要实现什么，还应告诉 AI 在实现安全之前缺少什么。

重要诊断包括：

- 缺少 `platform`。
- 缺少 `asset_format`。
- Figma 节点渲染不支持的渲染格式。
- 目标节点是宽泛的 `SECTION`、`CANVAS` 或用户流程页面，且 `scope.status=needs_narrowing`。
- 由于 token 预算导致的上下文截断。
- 缺少截图、截图 `null_count`、token 或图片 fill。

当 `scope.status=needs_narrowing` 时，AI 客户端应选择
`screen_candidates` 中 `implementable=true` 的节点并重新调用
`get_design_package`。fighorse 不会静默把容器替换成任意子 frame。

`visual_audit` 和 `project playbook` 将包扩展为完整的反馈循环。`visual_audit` 将 Figma URL 加可选的应用截图转化为结构化比较检查清单和经验建议。`project playbook` 将 AI 契约、输出策略、API 覆盖和本地经验组合为可复用的项目指令。

## 自发现契约

AI 客户端不应依赖陈旧的 prompt 文本。它们应调用：

- CLI：`fighorse discover --format json`
- MCP：`discover_fighorse`

发现 manifest 描述：

- 可用的 CLI 和 MCP 能力。
- REST 覆盖和官方 MCP 对比。
- 推荐的设计复刻工作流。
- 安全默认值。
- 本地写入要求。
- 经验存储行为。
- 当前 AI 契约。
- 安装和客户端配置提示。
- 支持它们的客户端的 MCP 资源和 prompt。

`doctor` 通过运行时状态补充发现：运行时信息、认证状态、home 目录、MCP 模式、本地写入模式和经验存储就绪状态。

## 自学习模型

fighorse 在追加式 JSONL 存储中保留可复用的经验：

- 全局：`~/.fighorse/experience/global.jsonl`。
- 项目：`./.fighorse/experience.jsonl`（在 `fighorse install project` 之后）。
- 精确覆盖：`FIGHORSE_EXPERIENCE_PATH`。
- 范围覆盖：`FIGHORSE_EXPERIENCE_SCOPE=global|project|merged`。

这是有意本地和透明的。目标不是自主的隐藏记忆；而是项目/用户拥有的实用设计复刻经验日志。

好的记录描述可复用的模式：

- 将重复 sibling 映射到堆叠容器导致的版式重叠。
- 紧凑组件排版需要直接检查而非全局缩放。
- 设备状态栏或安全区域不匹配。
- 资产格式或字体可用性约束。

记录不应包含密钥、私有绝对路径或一次性项目详情。

## 安全边界

fighorse 分离四个安全域：

- Figma 读取访问：需要令牌，是正常操作模式。
- Figma 写入访问：除非 `FIGHORSE_MCP_MODE=write`，否则禁用。
- 本地文件系统写入：在 MCP 中除非 `FIGHORSE_MCP_LOCAL_WRITE=allow`，否则禁用。
- Code Connect 模板代码外发：在 MCP 中除非 `FIGHORSE_MCP_CODE_CONNECT=allow`，否则禁用。

本地文件导出仍限制在批准的根目录：

- `./.fighorse/exports`
- `./assets/fighorse`
- `~/.fighorse/exports`

这种分离很重要，因为下载本地截图或图片 fill 远不如修改 Figma 敏感，而 Code Connect preview/publish 会把项目模板代码发送给 Figma。用户明确控制每个域。

默认 MCP 适配器是 official Rust `rmcp` 2.2 `StreamableHttpService` 与 `LocalSessionManager`。它提供互相独立的 stateful session，在分发前校验 Host 和 Origin，按标准 Streamable HTTP 返回 JSON 或 event-stream response，并在 SIGINT/SIGTERM 时 graceful shutdown。`/mcp` 的 event stream 不是 legacy SSE transport；`/sse` 和 `/messages` 不再服务，`--transport sse` 明确失败。显式兼容模式使用标准 rmcp stdio，不存在私有 frame 协议。

## 安装设计

`fighorse install` 默认生成可审查的工件，仅在 `--apply` 时修改用户级配置。安装器目标：

- `~/.fighorse` 下的 home/配置设置。
- `~/.fighorse/bin/fighorse` 下的二进制安装。
- `~/.fighorse/config.json` 中的认证存储。
- Cursor、Codex、Kimi、Claude、opencode 和通用 agent 的 MCP 客户端片段。
- AI 客户端的用户级 skill/rule。
- Streamable HTTP MCP 的 macOS launchd 和 Linux systemd 用户服务。

当原生客户端命令可用时，安装器代码优先使用它们。否则它写入带有备份的标准用户配置文件。

`fighorse install --default --apply` 默认仅 CLI，因为公共 CLI 不应让用户惊讶于后台服务或绑定端口。四客户端服务命令为 `fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply`，Claude 单独命令为 `fighorse install client --client claude --apply`。

原生 payload 分别是 Cursor `{"url":"http://127.0.0.1:9449/mcp"}`、Kimi `{"transport":"http","url":"http://127.0.0.1:9449/mcp"}`、Claude `{"type":"http","url":"http://127.0.0.1:9449/mcp"}`、Codex `[mcp_servers.fighorse]` 加同一 URL。

事务顺序是 `preflight -> backup -> binary -> service -> health_ready -> clients -> skills -> verified`；`/health` 与真实 `initialize`/`tools/list` 成功前不写客户端。manifest 记录 hash、backup、顺序和 `desired_absent: true`，rollback 逆序恢复托管文件和服务状态；skill conflict 原地保留并生成确定性备份。

Canonical 三目标是 `~/.agents/skills/fighorse/SKILL.md`（Cursor/Kimi/Codex）、`~/.claude/skills/fighorse/SKILL.md`（Claude）、`~/.cursor/rules/fighorse.mdc`（Cursor）。fresh install 的 service/stdio local write 为 deny，已有显式 allow 仅在迁移时保留。

## 生态系统定位

fighorse 借鉴现有工具的经验，同时选择不同的边界。

官方 Figma MCP 是规定性的和深度集成的。它可以暴露 Code to Canvas、自动 Code Connect 映射和设计系统搜索，但其行为不透明，与 Figma 的产品表面绑定。

Framelink 风格的 MCP 是描述性的和轻量的。它为 AI 提供版式和样式事实，而非生成的代码结构。这避免了用当前代码库可能不想要的框架决策"污染"上下文。

fighorse 默认为描述性路径：事实优于生成代码。当显式启用时，它仍然可以暴露更丰富的 Figma 元数据、现代 Code Connect 模板工作流和可写端点，但主要工作流是"精确上下文输入，项目原生实现输出"。

公共 REST OpenAPI 中不存在的产品能力必须写清路由，而非静默近似。原生 canvas 修改路由到 fighorse 本地插件桥。托管 Code to Canvas、自动 Code Connect 映射发现、Make 资源和其他 Remote MCP-only 工作流仍是 Figma 官方产品能力。fighorse 的 Code Connect 支持是独立的现代模板兼容层，使用观察到的 preview/publish/delete 服务协议；它不声称实现 Figma 产品内的自动映射。

## 数据保真度

Figma REST API 数据通常对实现事实是规范忠实的：

- 几何、尺寸、边界和坐标。
- 颜色、效果、描边、fill、圆角半径。
- 文本字符和样式元数据。
- Auto Layout 属性。
- Component 和 instance 关系。
- 渲染 URL 和图片 fill。

像素完美的浏览器或移动渲染仍然需要反馈循环，因为渲染引擎、字体可用性、抗锯齿、混合模式和系统 UI 与 Figma 不同。因此 fighorse 将结构化 JSON 与截图结合，并坚持视觉验证，而非声称一次通过就完美。

## 现场使用经验

一个真实的 Android Compose 原型暴露了几个实际需求：

- `design package` 应规范化 Figma URL 和节点 id。
- `smoke` 和 `diagnostics.status=ready` 是有用的就绪信号。
- 导出 manifest 对下游脚本比终端日志更可靠。
- 安全文件名如 `376_12995.png` 在应用构建系统中效果更好。
- 宽泛的流程节点应在实现前缩小到具体的 frame。
- 重复的行需要列表/线性容器；通用堆叠容器导致重叠。
- 紧凑的消息卡片和完整聊天卡片可能需要不同的排版。
- 移动屏幕需要滚动安全的版式。
- 字体可用性和状态栏/安全区域行为必须显式处理。
- AI 工具在实现前需要平台和资产格式选择。

这些经验直接塑造了设计包、导出 manifest、本地写入和经验存储契约。

## 非目标

fighorse 不旨在成为通用代码生成器。代码应由 AI agent 使用目标仓库的现有模式生成。

它也不旨在替代 Figma 自己的托管产品集成。fighorse 的原生画布修改是刻意限定在本地的能力：Rust 桥接服务、已配对的 Figma 开发插件、显式 `FIGHORSE_CANVAS_MODE=write` 写入开关、脚本执行开关 `FIGHORSE_CANVAS_SCRIPT=allow`，并且不调用私有服务端接口。对于自动 Code Connect 映射、托管 Code to Canvas、Make resources 或其他 Remote MCP-only 工作流，官方 MCP 仍然是合适的。

持久的产品边界是上下文和资产管道：获取、精简、解释、导出、验证和学习。

## 验证策略

该项目应保持无需真实 Figma 令牌即可验证：

- 单元与集成测试覆盖参数解析、精简、URL 解析、路径验证、OpenAPI 覆盖、操作调度、MCP 工具/资源路由、official HTTP/standard stdio、本地画布协议和会话门禁、发现 manifest、安装事务、设计包诊断和文档一致性。
- 接触真实 Figma API 的集成测试是可选的。
- 文档和生成的安装工件必须反映当前 CLI 行为。

指导维护规则是 CLI、MCP schema、安装器输出、发现 manifest、skills/rules 和正式文档之间的一致性。
