# fighorse

> [English](../en/README.md) | [中文](../zh/README.md) | [Русский](README.md)

Швейцарский армейский нож для данных Figma, заточенный под потребление AI.

`fighorse` — это Bun-first ClojureScript CLI и MCP Server. Он не генерирует код, а преобразует данные Figma REST API в стабильный, потребляемый контекст для инструментов AI-программирования и разработчиков: полный публичный REST API, дерево структуры, компактный JSON, URL скриншотов, дизайн-токены, экспорт изображений/компонентов, манифесты, информация самообнаружения и локальный опыт.

Ключевая идея — **CLI в ядре, MCP в оболочке**. CLI остается белым ящиком, скриптуемым и отлаживаемым; MCP позволяет Cursor, Codex, Kimi, Claude, opencode и другим AI-инструментам напрямую вызывать тот же набор возможностей.

## Быстрый старт

Путь по умолчанию — только CLI. Он не запускает долгоживущий MCP-сервис и не привязывает никакой порт.

```bash
bun install
bun run install:local
```

```bash
fighorse auth login --token <FIGMA_TOKEN>
fighorse quickstart
```

В Figma скопируйте ссылку на конкретный фрейм, компонент или группу, которую хотите изучить. Избегайте начинать со всей страницы/холста, если только вы не исследуете.

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
fighorse install --default --mode service --clients cursor,codex,kimi --apply
```

Упаковка распространяемых бинарников. Пакет по умолчанию — многоплатформенный бандл,
чей лаунчер `fighorse` автоопределяет macOS Intel, macOS Apple Silicon и
Linux x64/arm64:

```bash
bun run package
bun run package:macos
bun run package:linux
bun run package:darwin-universal
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
| Пакет дизайна | `design package`, `visual audit`, `project playbook`, `experience summary`, `experience add` |
| Данные Figma | `file get`, `file nodes`, `node get`, `file tree`, `file compact` |
| Ассеты | `image export`, `component export`, `asset download`, `images render`, `images fills` |
| Дизайн-система | `components`, `component-sets`, `styles`, `variables`, `tokens extract` |
| Установка | `install`, `install self`, `install home`, `install auth`, `install binary`, `install client`, `install service`, `install skill`, `install all` |
| MCP | `mcp serve --transport http`, `mcp serve --transport sse --host 127.0.0.1`, явный режим совместимости `stdio` |

## Безопасные значения по умолчанию

- Запись в Figma отключена, если не установлено `FIGHORSE_MCP_MODE=write`.
- Локальный экспорт файлов через MCP требует `FIGHORSE_MCP_LOCAL_WRITE=allow`.
- Пути экспорта ограничены `./.fighorse/exports`, `./assets/fighorse` и `~/.fighorse/exports`.
- Установленные AI-клиенты по умолчанию используют общий локальный HTTP MCP-эндпоинт `http://127.0.0.1:9449/mcp`; MCP-сервер использует singleton-лок для предотвращения дублирования долгоживущих процессов.
- Обычные CLI-команды остаются одноразовыми процессами: они не запускают MCP-сервис, не привязывают порты, не используют MCP singleton-лок. `fighorse install all` по умолчанию настраивает только CLI; используйте `--mode service` или `install service --apply` только когда явно хотите долгоживущий MCP-сервис.
- AI-клиенты должны спрашивать целевую платформу и формат ассетов при отсутствии; PNG — только резервный вариант рендеринга, не продуктовое решение.

## Разработка

```bash
bun run test
bun run build
bun run compile
bun run package
bun run install:local
bun run check
```

Тесты с реальным Figma API — опционально:

```bash
FIGMA_TOKEN=<token> bun run test:integration
```

## Лицензия

[1st Public License (1PL)](https://license.pub/1pl/)
