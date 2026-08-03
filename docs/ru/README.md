# fighorse

> [English](../en/README.md) | [中文](../zh/README.md) | [Русский](README.md)

Швейцарский армейский нож для данных Figma, заточенный под потребление AI.

`fighorse` — это Rust CLI и MCP Server. Он не генерирует код, а преобразует данные Figma REST API в стабильный, потребляемый контекст для инструментов AI-программирования и разработчиков: полный публичный REST API, дерево структуры, компактный JSON, URL скриншотов, дизайн-токены, экспорт изображений/компонентов, манифесты, информация самообнаружения и локальный опыт.

Ключевая идея — **CLI в ядре, MCP в оболочке**. CLI остается белым ящиком, скриптуемым и отлаживаемым; MCP позволяет Cursor, Codex, Kimi, Claude, opencode и другим AI-инструментам напрямую вызывать тот же набор возможностей.

## Быстрый старт

Путь по умолчанию — только CLI. Он не запускает долгоживущий MCP-сервис и не привязывает никакой порт.

```bash
cargo build --release
./target/release/fighorse install --default --apply --source ./target/release/fighorse
```

```bash
fighorse auth login --token <FIGMA_TOKEN>
fighorse quickstart
```

В Figma скопируйте ссылку на конкретный фрейм, компонент или группу, которую хотите изучить. Избегайте начинать со всей страницы/холста, если только вы не исследуете.

Чтобы перед выбором файла получить каталог всех доступных ресурсов команды
или проекта, используйте команду только для чтения:

```bash
fighorse resource catalog "https://www.figma.com/files/<root>/team/<team-id>"
```

В MCP ей соответствует `get_resource_catalog`. Отчёт содержит проекты, файлы,
ветки и библиотеки команды, а состояние задаётся как `ready`, `partial` или
`blocked`. Для проектов нужен `projects:read`, для библиотек —
`team_library_content:read`, для опциональной проверки
`--probe-file-access` — `file_content:read`. По ссылке
`/files/<browser-root>` публичный REST API не может определить команду.

```bash
fighorse quickstart "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
```

Сгенерируйте пакет контекста, необходимый для воспроизведения дизайна:

```bash
fighorse design package "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>" \
  --platform <target-platform> \
  --asset-format <asset-format>
```

Экспортируйте визуальные ассеты:

```bash
fighorse image export <file_key> --ids 1:2,1:3 --dir ./.fighorse/exports --manifest
fighorse component export <file_key> --ids 2:8 --format svg --dir ./assets/fighorse --manifest
fighorse asset download <file_key> --dir ./assets/fighorse --manifest
```

Дополнительный MCP-режим сервиса для AI-клиентов:

```bash
fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply
fighorse install verify
# Только Claude:
fighorse install client --client claude --apply
```

Установка сервиса транзакционна: сначала записываются бинарник и сервис, затем проверяется `/health` и выполняются реальные `initialize` и `tools/list` на `/mcp`; только после этого записываются конфиги клиентов и skills. `~/.fighorse/install/manifest.json` хранит managed-файлы и удаления `desired_absent`, а исходные данные и конфликты — `~/.fighorse/install/backups/`. `fighorse install rollback` восстанавливает неизменённые managed-файлы и прежнее состояние сервиса.

Нативные HTTP payload: Cursor `{"url":"http://127.0.0.1:9449/mcp"}`, Kimi `{"transport":"http","url":"http://127.0.0.1:9449/mcp"}`, Claude `{"type":"http","url":"http://127.0.0.1:9449/mcp"}`, Codex — `[mcp_servers.fighorse]` с тем же URL. Конфигурация Codex заранее разрешает только read-only инструменты `discover_fighorse` и `get_resource_catalog`, чтобы headless-сеансы могли выполнить самодиагностику и инвентаризацию browser-ссылки; все остальные MCP-инструменты сохраняют обычный режим подтверждений Codex.

Три canonical-цели: `~/.agents/skills/fighorse/SKILL.md` для Cursor/Kimi/Codex, `~/.claude/skills/fighorse/SKILL.md` для Claude и `~/.cursor/rules/fighorse.mdc` для Cursor.

Упаковка распространяемых бинарников с помощью Cargo. Кросс-компилируйте под каждую
цель с соответствующим Rust-тулчейном (или `cargo-zigbuild` для Linux-целей):

```bash
cargo build --release
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu
```

## Документация

- [Быстрый старт](quickstart.md): первый успешный запуск CLI, ссылка на фрейм, пакет дизайна, дополнительная настройка MCP.
- [Руководство пользователя](user-guide.md): установка, аутентификация, CLI, MCP-сервис, локальный экспорт ассетов, хранение опыта, устранение неполадок.
- [Руководство AI-клиента](ai-client-guide.md): как AI-инструменты должны самообнаруживаться, вызывать MCP/CLI, экспортировать ассеты, запрашивать платформу/формат ассетов и записывать переиспользуемые уроки.
- [Архитектура](design.md): архитектура, цели продукта, компромиссы экосистемы, модель самообнаружения/самообучения, границы безопасности.

## Основные команды

| Область | Команды |
|------|----------|
| Обнаружение | `discover`, `doctor`, `smoke`, `url parse`, `mcp config` |
| Официальный REST | `figma-api coverage`, `figma api <operationId>` |
| Code Connect | `code-connect generate`, `code-connect parse`, `code-connect validate`, `code-connect preview`, `code-connect publish`, `code-connect unpublish` |
| Пакет дизайна | `design package`, `visual audit`, `project playbook`, `experience summary`, `experience add` |
| Данные Figma | `file get`, `file nodes`, `node get`, `file tree`, `file compact` |
| Ассеты | `image export`, `component export`, `asset download`, `images render`, `images fills` |
| Дизайн-система | `components`, `component-sets`, `styles`, `variables`, `tokens extract` |
| Установка | `install`, `install self`, `install home`, `install auth`, `install binary`, `install client`, `install service`, `install skill`, `install all`, `install verify`, `install rollback` |
| MCP | `mcp serve --transport http`, явный режим совместимости `stdio` |

## Безопасные значения по умолчанию

- Запись в Figma отключена, если не установлено `FIGHORSE_MCP_MODE=write`.
- Локальный экспорт файлов через MCP требует `FIGHORSE_MCP_LOCAL_WRITE=allow`.
- MCP preview/publish для Code Connect требует `FIGHORSE_MCP_CODE_CONNECT=allow`; publish/unpublish также требует `FIGHORSE_MCP_MODE=write`.
- Пути экспорта ограничены `./.fighorse/exports`, `./assets/fighorse` и `~/.fighorse/exports`.
- Установленные AI-клиенты по умолчанию используют общий локальный HTTP MCP-эндпоинт `http://127.0.0.1:9449/mcp`; MCP-сервер использует singleton-лок для предотвращения дублирования долгоживущих процессов.
- `/mcp` реализован official Rust `rmcp` 2.2 Streamable HTTP: независимые stateful sessions, проверка Host/Origin, JSON или event-stream response и graceful shutdown. Legacy `/sse` и `/messages` отсутствуют; `--transport sse` завершается ошибкой с переходом на `--transport http`.
- Новые service и явные stdio-конфиги используют `FIGHORSE_MCP_LOCAL_WRITE=deny`; существующий явный `allow` сохраняется только при миграции.
- Обычные CLI-команды остаются одноразовыми процессами: они не запускают MCP-сервис, не привязывают порты, не используют MCP singleton-лок. `fighorse install all` по умолчанию настраивает только CLI; используйте `--mode service` или `install service --apply` только когда явно хотите долгоживущий MCP-сервис.
- AI-клиенты должны спрашивать целевую платформу и формат ассетов при отсутствии; PNG — только резервный вариант рендеринга, не продуктовое решение.

## Code Connect

fighorse нативно поддерживает современные parserless-шаблоны Code Connect (`.figma.ts`, `.figma.js` и `.figma.batch.json`) без Node.js и без official Code Connect CLI. Он генерирует шаблоны из явно переданного AI-контекста кода, парсит их локально без выполнения, проверяет Figma component nodes, запускает настоящий удаленный preview, publish и unpublish.

```bash
fighorse code-connect generate "<figma-component-url>" --context code-context.json
fighorse code-connect parse --dir .
fighorse code-connect preview --documents docs.json
fighorse code-connect publish --documents docs.json --dry-run
fighorse code-connect publish --documents docs.json --yes --force
fighorse code-connect unpublish --node "<figma-component-url>" --label React --dry-run
```

Automatic Code Connect mapping discovery остается возможностью продукта Figma; используйте official Figma Remote MCP, когда нужен именно этот workflow.

## Разработка

```bash
cargo test
cargo build --release
cargo clippy
```

Тесты с реальным Figma API — опционально:

```bash
FIGMA_INTEGRATION_TESTS=1 FIGMA_TOKEN=<token> cargo test -- --ignored
```

## Лицензия

[1st Public License (1PL)](https://license.pub/1pl/) (полный текст в файле [LICENSE](../../LICENSE))
