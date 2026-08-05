# Руководство AI-клиента fighorse

Это руководство для AI-инструментов кодирования и агентов, использующих fighorse через MCP или CLI. Контракт прост: сначала обнаружение, спрашивайте при отсутствии продуктовых предположений, экспортируйте ассеты с манифестами, запускайте визуальный feedback loop и записывайте переиспользуемые уроки.

## Маршрутизация браузерных ссылок

Для ссылки браузера команды или проекта сначала вызывайте readonly
`get_resource_catalog`, а не просите пользователя открывать каждый файл.
Сохраняйте диагностику `ready`/`partial`/`blocked`; после выбора конкретного
файла вызывайте `get_design_package`. Не считайте `/files/<browser-root>` ID
команды и не записывайте ID, имена, ключи, URL или приватное содержимое
каталога в переиспользуемый опыт. Сначала читайте поля `parse_figma_url`:
`catalog_eligible=true` разрешает каталог, а `browser_root_not_enumerable`
означает, что нужно попросить URL команды/проекта или конкретный design URL.

## Настройка клиента

По возможности используйте установщик:

```bash
fighorse install client --client cursor --apply
fighorse install client --client codex --apply
fighorse install client --client kimi --apply
fighorse install client --client claude --apply
fighorse install skill --clients cursor,codex,kimi,claude --apply
```

Для генерации конфигов без применения:

```bash
fighorse mcp config --client cursor --transport http
fighorse mcp config --client codex --transport http
fighorse mcp config --client kimi --transport http
fighorse mcp config --client claude --transport http
fighorse mcp config --client opencode --transport http
```

Рекомендуемая установленная MCP-конфигурация:

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

**Почему оба?** fighorse обрабатывает read-workflows от дизайна к коду (пакет дизайна, экспорт ассетов, визуальный аудит, обучение на опыте) и современные Code Connect template workflows. Официальный Figma Remote MCP обрабатывает product-only возможности, не предоставляемые публичным REST: нативные canvas-записи, Code to Canvas, авто-привязку Code Connect, генерацию FigJam и ресурсы Make. Они дополняют друг друга и могут сосуществовать в одном клиенте.

- Официальный Remote MCP: `https://mcp.figma.com/mcp` — OAuth-аутентификация, бесплатно в бета-версии, станет платным по мере использования.
- Требования к местам: Full seat для записи в общие файлы; Dev seat — только чтение вне черновиков.

Рекомендуемый локальный сервис:

```bash
fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply
fighorse install verify
```

Подключайтесь к `http://127.0.0.1:9449/mcp` через official Rust `rmcp` 2.2 Streamable HTTP. Сервис хранит независимые stateful sessions, проверяет Host/Origin, согласует JSON или event-stream response и выполняет graceful shutdown. Legacy `/sse` и `/messages` отсутствуют; `--transport sse` завершается ошибкой с переходом на HTTP. `text/event-stream` от `/mcp` — standard response, не legacy transport.

Инсталлятор активирует service, ожидает `/health`, выполняет `initialize`/`tools/list`, затем пишет clients и skills. Manifest, backup и `desired_absent` находятся в `~/.fighorse/install/manifest.json` и `~/.fighorse/install/backups/`; rollback восстанавливает неизменённые managed files и прежний service state.

Canonical-цели: `~/.agents/skills/fighorse/SKILL.md` для Cursor/Kimi/Codex, `~/.claude/skills/fighorse/SKILL.md` для Claude и `~/.cursor/rules/fighorse.mdc` для Cursor.

## Обязательный стартовый поток

При подключении к fighorse делайте это перед реализацией:

1. Вызовите `discover_fighorse`.
2. Вызовите `doctor` или прочитайте `discover_fighorse.production_defaults`.
3. При необходимости разберите URL Figma, предоставленный пользователем, с помощью `parse_figma_url`.
4. Если отсутствует целевая платформа или формат ассетов, спросите разработчика.
5. Вызовите `list_experiences` для соответствующей платформы, формата ассетов, file key и типа узла.
6. Вызовите `get_design_package` с URL Figma, платформой и форматом ассетов.
7. Экспортируйте необходимые изображения/компоненты/fill с `manifest: true`, если нужны локальные ассеты.
8. Реализуйте UI в целевой кодовой базе.
9. Запустите приложение, сделайте скриншоты, сравните с Figma-референсами и вызовите `visual_audit` для структурированного руководства по несоответствиям.
10. Вызовите `record_experience` для переиспользуемых уроков, обнаруженных во время дебаггинга.

Не пропускайте самообнаружение. Манифест является частью API-контракта и может развиваться быстрее, чем ручные инструкции клиента.

## Спрашивайте вместо догадок

Спросите разработчика, когда что-либо из этого отсутствует:

- Целевая платформа: web, Android Compose, iOS SwiftUI/UIKit, React Native, Flutter, desktop и т.д.
- Формат ассетов: png, svg, pdf, jpg, webp или платформенно-специфичный векторный формат.
- Область: конкретный screen/frame против широкого CANVAS/узла пользовательского потока.
- Место назначения для production-ассетов, когда `./assets/fighorse` не подходит.

PNG — только самый безопасный резервный вариант рендеринга узлов Figma. Это не продуктовое решение по умолчанию.

## Рекомендуемые MCP-инструменты

Сначала используйте эти высокоуровневые инструменты:

- `discover_fighorse`: возможности, контракты, безопасные значения по умолчанию, рекомендуемый workflow.
- `doctor`: статус runtime/аутентификации/локальной записи.
- `parse_figma_url`: нормализация file key и node id.
- `get_design_package`: структурированный пакет реализации.
- `list_experiences`: переиспользуемые локальные уроки.
- `record_experience`: запись переиспользуемых уроков.
- `visual_audit`: структурированное сравнение скриншотов, анализ несоответствий и предложения по опыту.
- `get_project_playbook`: сборка проектно-уровневых правил реализации из руководств и локального опыта.

Используйте эти инструменты для ассетов:

- `export_images`: рендеринг скриншотов/слайсов узлов.
- `export_component`: экспорт узлов component/control как png/svg/pdf/jpg.
- `download_image_fills`: загрузка image fills, на которые ссылается дизайн.

Используйте низкоуровневые Figma-инструменты только когда пакета дизайна недостаточно:

- `get_file_compact`
- `get_node`
- `get_file_tree`
- `get_image`
- `get_image_fills`
- `get_file_tokens`

Используйте сгенерированные официальные REST-инструменты, когда нужна точная OpenAPI-паритетность. Эти инструменты называются `figma_<operation_id_in_snake_case>`, например `figma_get_file`, `figma_get_developer_logs`, `figma_put_webhook` и `figma_post_variables`. В readonly MCP-режиме сгенерированные инструменты записи Figma скрыты и заблокированы; устанавливайте `FIGHORSE_MCP_MODE=write` только когда разработчик явно разрешает мутации Figma.

Используйте Code Connect tools только когда пользователь связывает code components с Figma Dev Mode:

- `parse_code_connect_template`: проверяет уже переданные Code Connect documents.
- `validate_code_connect`: проверяет, что целевые Figma nodes являются components или component sets.
- `preview_code_connect`: отправляет template code в Figma для реального snippet rendering; требует `FIGHORSE_MCP_CODE_CONNECT=allow`.
- `publish_code_connect`: публикует mappings; требует `FIGHORSE_MCP_CODE_CONNECT=allow` и `FIGHORSE_MCP_MODE=write`.
- `unpublish_code_connect`: удаляет точные node+label mappings; требует те же два переключателя.

Для генерации шаблонов AI-клиент должен читать целевой репозиторий своими файловыми инструментами, затем передавать явный component context в CLI `fighorse code-connect generate`. fighorse не сканирует и не выполняет пользовательский код через MCP.

Клиенты, поддерживающие MCP-ресурсы и промпты, также могут читать:

- `fighorse://capabilities`
- `fighorse://coverage`
- `fighorse://workflow/design-replication`
- `fighorse://experience/summary`
- Промпт: `fighorse_design_replication`
- Промпт: `fighorse_api_coverage`

## CLI-эквиваленты

Если MCP недоступен, запустите эквивалентные CLI-команды:

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

## Политика локальной записи

Режим записи MCP Figma и режим записи локальной файловой системы независимы.

- `FIGHORSE_MCP_MODE=readonly`: по умолчанию; инструменты записи Figma не предоставляются.
- `FIGHORSE_MCP_MODE=write`: предоставляет инструменты записи Figma там, где реализованы.
- `FIGHORSE_MCP_LOCAL_WRITE=deny`: по умолчанию; локальные инструменты экспорта заблокированы.
- `FIGHORSE_MCP_LOCAL_WRITE=allow`: разрешает локальные инструменты экспорта в пределах одобренных корней.
- `FIGHORSE_MCP_CODE_CONNECT=deny`: по умолчанию; MCP не может отправлять Code Connect template code в Figma.
- `FIGHORSE_MCP_CODE_CONNECT=allow`: разрешает Code Connect preview/publish template egress в Figma.

Одобренные корни:

- `./.fighorse/exports`
- `./assets/fighorse`
- `~/.fighorse/exports`

Всегда запрашивайте `manifest: true` для инструментов экспорта. Читайте манифест для определения расположения файлов вместо вывода имен файлов.

## Контракт пакета дизайна

Рассматривайте `get_design_package` как источник истины для реализации. Важные поля:

- `implementation_target`: предположения о платформе и формате ассетов плюс предупреждения.
- `target`: идентификация выбранного узла, тип, размеры и вероятность, что он слишком широк.
- `scope`: `ready_to_implement` или `needs_narrowing`.
- `screen_candidates` и `component_candidates`: вероятные фреймы/компоненты для изучения, сужения или экспорта; для сужения используйте кандидаты с `implementable=true`.
- `context`: компактные данные дизайна для реализации.
- `tokens`: извлеченные цвета, типографика, отступы и эффекты.
- `token_confidence` и `missing_font_diagnostics`: сигналы качества для надежности токенов/шрифтов.
- `screenshots`: URL рендеринга, возвращенные Figma.
- `asset_export_plan`: точные следующие команды экспорта и MCP-вызовы.
- `learned_experience`: уроки из предыдущих запусков.
- `implementation_risk_checklist`: конкретные риски для проверки перед финализацией.
- `diagnostics`: статус готовности, предупреждения и screenshot `null_count`.

Если `SECTION`, `CANVAS`, `DOCUMENT` или `SELECTION` возвращает
`scope.status=needs_narrowing`, снова вызовите `get_design_package` с узлом
`screen_candidates`, где `implementable=true`. Если `diagnostics.status` не
`ready`, следуйте предупреждениям перед кодированием, когда возможно.

## Цикл визуальной точности

Реализация не завершается после первой генерации кода. Используйте этот цикл:

1. Экспортируйте референсные скриншоты или слайсы из fighorse.
2. Реализуйте целевой экран, используя точные размеры, типографику, отступы и цвета из пакета.
3. Запустите приложение на целевой платформе.
4. Сделайте скриншот в нужном viewport/размере устройства.
5. Сравните скриншот приложения с Figma-референсом.
6. Исправьте проблемы с layout, типографикой, ассетами, обрезкой, скроллом, статус-баром и наложениями.
7. Повторяйте, пока различия не будут поняты и приемлемы.

Известные уроки из реального использования:

- Повторяющиеся sibling должны отображаться на list/linear-контейнеры платформы, а не на generic stacking-контейнеры.
- Компактные карточки часто нуждаются в собственном размере шрифта и высоте строки; не используйте типографику всей карточки слепо.
- Мобильные экраны требуют scroll-safe layout-решений вместо фиксированных вертикальных стеков.
- Системный UI реального устройства может перекрывать статус-бар Figma, если явно не задан fullscreen или safe-area handling.
- Отсутствующие шрифты должны диагностироваться и обрабатываться осознанно.

## Запись опыта

Записывайте уроки, которые помогут будущим, несвязанным задачам. Хороший опыт — платформенно-осведомленный, но не проектно-специфичный:

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

Не записывайте:

- Секреты, токены, приватные URL или локальные абсолютные пути.
- Разовые проектные решения, которые не переиспользуются.
- Уроки, которые просто пересказывают содержание дизайна.

## Настройка под конкретные клиенты

Все клиенты должны получать один и тот же fighorse-контракт. Различия должны ограничиваться формой конфигурационного файла и транспортом. Рекомендуемая публичная настройка — один общий локальный HTTP MCP-сервис на `http://127.0.0.1:9449/mcp`.

### Cursor

Установка:

```bash
fighorse install --default --mode service --clients cursor --apply
```

Ожидаемая форма конфигурации:

```json
{
  "mcpServers": {
    "fighorse": {
      "url": "http://127.0.0.1:9449/mcp"
    }
  }
}
```

Проверка:

```bash
fighorse quickstart --format json
fighorse doctor --format json
```

Типичная проблема: Cursor настроен на повторное порождение stdio. Замените эту конфигурацию на общий HTTP-эндпоинт, если только клиент не может подключиться к localhost HTTP.

### Codex

Установка:

```bash
fighorse install --default --mode service --clients codex --apply
```

Ожидаемый сгенерированный TOML:

```toml
[mcp_servers.fighorse]
url = "http://127.0.0.1:9449/mcp"
enabled = true
startup_timeout_sec = 60
```

Проверка:

```bash
fighorse install status
curl http://127.0.0.1:9449/health
```

Типичная проблема: Codex может создавать новую Streamable HTTP session при запуске. `/mcp` должен создавать независимую stateful session и возвращать standard JSON или event-stream response.

### Kimi

Установка:

```bash
fighorse install --default --mode service --clients kimi --apply
```

Ожидаемая форма команды:

```bash
kimi mcp add --transport http fighorse http://127.0.0.1:9449/mcp
```

Ожидаемый payload: `{"transport":"http","url":"http://127.0.0.1:9449/mcp"}`.

Проверка:

```bash
fighorse quickstart --format json
```

Типичная проблема: старые Kimi-клиенты могут поддерживать только stdio. Используйте `fighorse mcp config --client kimi --transport stdio` только для этого случая совместимости.

### Claude

Генерация или установка:

```bash
fighorse install client --client claude --apply
fighorse mcp config --client claude --transport http
```

Ожидаемая форма конфигурации:

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

Проверьте, попросив Claude вызвать `discover_fighorse`, затем `check_fighorse_ready`.

Типичная проблема: desktop/клиентское окружение может не наследовать shell-токены. Сохраните токен с помощью `fighorse auth login --token <FIGMA_TOKEN>`, чтобы сервис мог читать локальную конфигурацию.

### opencode

Установка или генерация:

```bash
fighorse install client --client opencode --apply
fighorse mcp config --client opencode --transport http
```

Ожидаемая форма конфигурации — тот же HTTP MCP-элемент:

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

Проверка:

```bash
fighorse doctor --format json
```

Типичная проблема: режим сервиса не был установлен, потому что `install all` по умолчанию настраивает только CLI. Перезапустите с `--mode service`.

### VS Code-совместимые клиенты

Используйте generic HTTP MCP-конфиг, если только клиент не документирует другую схему:

```bash
fighorse mcp config --client generic --transport http
```

Ожидаемая форма:

```json
{
  "fighorse": {
    "transport": "http",
    "url": "http://127.0.0.1:9449/mcp"
  }
}
```

Типичная проблема: клиент ожидает обертку `mcpServers`. Если так, используйте форму Cursor-style выше.

### Generic MCP

Для Streamable HTTP:

```json
{
  "transport": "http",
  "url": "http://127.0.0.1:9449/mcp"
}
```

Только для явной совместимости с stdio:

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

Типичная проблема: множество долгоживущих stdio-процессов потребляют ресурсы. Предпочитайте общий HTTP-сервис для клиентов, которые его поддерживают.

Когда AI-инструмент видит URL Figma и fighorse доступен, он не должен вручную скрейпить URL, угадывать frame id или реализовывать из визуальной памяти. Сначала используйте fighorse.

Для native canvas writes используйте локальный plugin bridge, а не REST token:
`canvas_status`, `canvas_create_pairing`, `canvas_list_sessions`,
`canvas_apply`, `canvas_verify` и `canvas_undo`. Пользователь должен установить
и запустить Figma plugin. Запись требует `FIGHORSE_MCP_MODE=write`,
`FIGHORSE_CANVAS_MODE=write` и `yes=true`; scripts дополнительно требуют
`FIGHORSE_CANVAS_SCRIPT=allow`. Если подключено несколько sessions, спросите
или передайте точный `session_id`. Если result равен `unknown`, сначала
выполните inspect или verify.
