# Быстрый старт fighorse

Это руководство помогает новому пользователю от нуля до полезного контекста Figma как можно быстрее. Начните с CLI-режима. Добавьте MCP-режим сервиса только когда хотите, чтобы AI-клиент напрямую вызывал fighorse.

## 1. Установка

Сборка из исходников:

```bash
cargo build --release
./target/release/fighorse install --default --apply --source ./target/release/fighorse
```

`install --default --apply` копирует бинарник в дом fighorse, генерирует локальную конфигурацию и устанавливает skills/инструкции fighorse — тот же путь самоустановки, который используется упакованным бинарником.

Установка скачанного бинарника:

```bash
./fighorse install --default --apply
```

Установка в произвольную директорию:

```bash
./fighorse install --path ~/.local/bin --apply
```

Эти команды по умолчанию настраивают только CLI. Они устанавливают бинарник и локальный дом fighorse, но не запускают MCP-сервис и не привязывают порт.

## 2. Добавление Figma-токена

Создайте персональный токен доступа Figma с правами на чтение содержимого файлов. Затем сохраните его локально:

```bash
fighorse auth login --token <FIGMA_TOKEN>
```

Также можно не хранить токен в конфиге и запускать разовые команды с `FIGMA_TOKEN=<token>`.

## 3. Проверка настройки

Запустите интерактивную проверку:

```bash
fighorse quickstart
```

Для машиночитаемого вывода:

```bash
fighorse quickstart --format json
```

## 4. Копирование конкретной ссылки Figma

В Figma выберите конкретный фрейм, компонент или группу, который хотите реализовать. Скопируйте ссылку на это выделение. Избегайте начинать со всей страницы или широкого холста, если только вы не исследуете.

Проверьте ссылку:

```bash
fighorse quickstart "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
```

## 5. Получение пакета дизайна

Сначала уточните целевую платформу и формат ассетов. Затем соберите пакет:

```bash
fighorse design package "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>" \
  --platform web-react \
  --asset-format svg \
  --output ./.fighorse/exports/package.json
```

Это основной источник контекста для AI-реализации. Он включает компактную структуру, скриншоты, токены, диагностику, рекомендации по экспорту ассетов и локальный накопленный опыт.

## 6. Дополнительно: MCP-режим сервиса

Используйте режим сервиса только когда AI-клиент должен напрямую вызывать fighorse:

```bash
fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply
fighorse install verify
# Только Claude:
fighorse install client --client claude --apply
```

Установленные клиенты должны использовать:

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

Сервис по умолчанию доступен только на localhost и защищён singleton-локом. `/mcp` использует official Rust `rmcp` 2.2 Streamable HTTP: независимые stateful sessions, проверку Host и Origin, JSON или event-stream response и graceful shutdown.

Нативные payload: Cursor `{"url":"http://127.0.0.1:9449/mcp"}`, Kimi `{"transport":"http","url":"http://127.0.0.1:9449/mcp"}`, Claude `{"type":"http","url":"http://127.0.0.1:9449/mcp"}`, Codex — `[mcp_servers.fighorse]`. Порядок установки: service → `/health` → `initialize`/`tools/list` → clients → skills. Manifest/backup обеспечивают `install verify` и `install rollback`; managed-удаления записываются как `desired_absent`.

Canonical-цели: `~/.agents/skills/fighorse/SKILL.md` для Cursor/Kimi/Codex, `~/.claude/skills/fighorse/SKILL.md` для Claude и `~/.cursor/rules/fighorse.mdc` для Cursor.

Legacy `/sse` и `/messages` не обслуживаются, а `--transport sse` завершается ошибкой с переходом на HTTP. `text/event-stream` от `/mcp` — стандартный Streamable HTTP response, а не legacy SSE transport. Новая установка запрещает local write.

## 7. Дополнительно: Code Connect

Для современных Code Connect templates:

```bash
fighorse code-connect generate "<figma-component-url>" --context code-context.json
fighorse code-connect parse --dir .
fighorse code-connect publish --documents docs.json --dry-run
```

Используйте `--yes` только после проверки dry-run output. MCP preview/publish также требует `FIGHORSE_MCP_CODE_CONNECT=allow`; publish/unpublish требует `FIGHORSE_MCP_MODE=write`.

## 8. Что спросить у вашего AI-агента

После подключения MCP вставьте конкретную ссылку на фрейм Figma и спросите:

```text
Используй fighorse для изучения этого фрейма Figma. Сначала вызови discover_fighorse, затем list_experiences, потом get_design_package. Спроси меня про платформу или формат ассетов, если они не указаны. Экспортируй ассеты с манифестами вместо догадок.
```

## Устранение неполадок

- Отсутствует токен: запустите `fighorse auth login --token <FIGMA_TOKEN>`.
- Слишком широкая ссылка: скопируйте ссылку на выделенный фрейм или компонент.
- MCP-сервис не запущен: используйте `fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply`.
- Codex сообщает о неожиданном типе контента: запустите `fighorse install verify`; `/mcp` должен возвращать стандартный MCP JSON или event-stream response, а не product manifest.
- Локальный экспорт отклонен: используйте `./.fighorse/exports`, `./assets/fighorse` или `~/.fighorse/exports`.
