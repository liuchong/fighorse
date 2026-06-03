# Руководство пользователя fighorse

`fighorse` — это public-first, open-source Figma CLI + MCP. Он преобразует данные публичного Figma REST API в AI-дружественный контекст и developer-friendly CLI-вывод. Первый запуск должен быть простым: установка, добавление токена, вставка конкретной ссылки на фрейм и генерация полезного пакета дизайна. Второй запуск может углубиться в манифесты ассетов, визуальный аудит, project playbooks, полное покрытие REST и локальное обучение на опыте.

Используйте CLI, когда нужны воспроизводимые команды, скрипты, CI или быстрая инспекция. Используйте MCP-сервис, когда AI-инструмент кодирования должен напрямую вызывать fighorse.

## Установка из исходников

Самый быстрый путь — см. [Быстрый старт](quickstart.md). Режим установки по умолчанию — только CLI, без запуска MCP-сервисных процессов.

```bash
bun install
bun run install:local
./dist/fighorse --help
```

Установка скомпилированного бинарника и опциональных интеграций AI-клиентов:

```bash
./dist/fighorse install status
./dist/fighorse install auth --apply
./dist/fighorse install --default --apply
```

Установка MCP-сервиса и интеграций AI-клиентов только при необходимости:

```bash
./dist/fighorse install client --client cursor --apply
./dist/fighorse install client --client codex --apply
./dist/fighorse install client --client kimi --apply
./dist/fighorse install service --service launchd --apply
./dist/fighorse install --default --mode service --clients cursor,codex,kimi --apply
```

Команды установки по умолчанию генерируют файлы для ревью. Добавляйте `--apply` только когда хотите, чтобы fighorse модифицировал пользовательскую конфигурацию клиента, расположения skill/rule, ссылки на бинарники или сервис-менеджеры.

## Упаковка бинарников

Команда package по умолчанию собирает многоплатформенный бандл. Его топ-левел
лаунчер `fighorse` определяет `uname -s` и `uname -m`, затем запускает подходящий
нативный бинарник для macOS Intel, macOS Apple Silicon, Linux x64 или Linux arm64:

```bash
bun run package
tar -tzf dist/fighorse-multi-platform.tar.gz
```

Также можно собирать архивы под конкретные платформы:

```bash
bun run package:macos
bun run package:darwin-x64
bun run package:darwin-arm64
bun run package:linux-x64
bun run package:linux-arm64
bun run package:all
```

`package:macos` создает два архива для архитектур macOS плюс macOS-бандл,
автоопределяющий Intel против Apple Silicon. Если вам конкретно нужен один
Mach-O файл, содержащий оба slice macOS, запустите `bun run package:darwin-universal`
на macOS с доступным `lipo`.

## Аутентификация

Создайте персональный токен доступа Figma в настройках разработчика Figma, затем сохраните его локально:

```bash
fighorse auth login --token <FIGMA_TOKEN>
```

Также можно использовать переменные окружения для разовых команд:

```bash
FIGMA_TOKEN=<FIGMA_TOKEN> fighorse file tree <file_key>
```

`install auth --apply` читает `--token`, stdin, `FIGMA_TOKEN` или `FIGMA_API_KEY` и сохраняет токен в `~/.fighorse/config.json`. Вывод команды маскирует токен.

## Проверка готовности

```bash
fighorse quickstart
fighorse quickstart "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
fighorse doctor --format json
fighorse discover --format json
fighorse smoke "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>"
fighorse figma-api coverage --format json
```

`smoke` использует реальный доступ к Figma и возвращает `fighorse.smoke.v1`. `ok: true` означает, что обычный путь пакета дизайна готов. `ok: false` с `diagnostics.status: partial` все еще может означать, что доступ работал; следуйте предупреждениям, обычно указывая платформу, формат ассетов или конкретный узел фрейма.

`figma-api coverage` сообщает о паритете с vendored снимком Figma REST OpenAPI. Текущий реестр отслеживает 48 публичных операций и предоставляет каждую через API-слой, generic CLI dispatch и сгенерированные официальные MCP-инструменты там, где это безопасно.

## Покрытие официального REST API

Используйте продуктовые команды для обычной дизайн-работы и generic REST dispatch, когда нужна точная низкоуровневая паритетность API:

```bash
fighorse figma-api coverage --format md
fighorse figma api getFile --params '{"file_key":"<file_key>","depth":1}'
fighorse figma api putWebhook --params '{"webhook_id":"<id>"}' --body '{"status":"PAUSED"}' --yes
```

`figma api` принимает официальные имена `operationId` из OpenAPI-реестра. Операции чтения выполняются нормально. Операции записи требуют `--yes`, потому что они могут модифицировать комментарии, переменные, вебхуки или dev-ресурсы.

## Изучение дизайнов

Разбор вставленного URL Figma:

```bash
fighorse url parse "https://www.figma.com/design/<fileKey>/<name>?node-id=1-2"
```

Просмотр структуры:

```bash
fighorse file tree <file_key> --depth 2
fighorse node get <file_key> <node_id> --depth 3
```

Получение компактного AI-контекста:

```bash
fighorse file compact <file_key> --depth 2 --max-tokens 8000
fighorse file get <file_key> --depth 2 | fighorse compact --max-tokens 8000
```

Извлечение дизайн-токенов:

```bash
fighorse file tokens <file_key> --format json
fighorse tokens extract <file_key> --format css --output ./tokens.css
```

## Сборка пакета дизайна

Для AI-реализации предпочитайте `design package` низкоуровневым вызовам:

```bash
fighorse design package "https://www.figma.com/design/<fileKey>/<name>?node-id=<node-id>" \
  --platform android-compose \
  --asset-format png \
  --max-tokens 8000 \
  --output ./.fighorse/exports/package.json
```

Пакет включает:

- `source`: разобранный file key и node id.
- `file` и `target`: метаданные и сводка по выбранному узлу.
- `implementation_target`: предположения о платформе и формате ассетов.
- `screen_candidates` и `component_candidates`: вероятные фреймы/компоненты для изучения или экспорта.
- `fidelity_workflow`: шаги визуальной верификации.
- `asset_export_plan`: предложенные CLI и MCP вызовы экспорта ассетов.
- `learned_experience`: локальные уроки из предыдущих запусков.
- `token_confidence`, `missing_font_diagnostics` и `implementation_risk_checklist`: AI-готовые проверки перед кодированием.
- `context`, `tokens`, `screenshots` и опциональные `assets`.
- `diagnostics`: предупреждения об отсутствующей платформе, формате ассетов, CANVAS-целях, усечении, отсутствующих скриншотах или токенах.

Если платформа или формат ассетов неизвестны, спросите разработчика перед реализацией. PNG — резервный вариант рендеринга, не автоматическое продуктовое решение.

## Экспорт ассетов

Используйте локальный экспорт, когда реализации нужны реальные файлы изображений вместо временных URL рендеринга:

```bash
fighorse image export <file_key> --ids <node_ids> --format png --dir ./.fighorse/exports --manifest
fighorse component export <file_key> --ids <component_node_ids> --format svg --dir ./assets/fighorse --manifest
fighorse asset download <file_key> --dir ./assets/fighorse --manifest
```

Рекомендуемые расположения вывода:

- `./.fighorse/exports`: временные слайсы, скриншоты, манифесты и отладочные ассеты.
- `./assets/fighorse`: ассеты, предназначенные для использования в коде приложения или упаковки.
- `~/.fighorse/exports`: кросс-проектные временные экспорты.

Команды экспорта пишут безопасные имена файлов и могут создавать `manifest.json`. Используйте манифест вместо парсинга терминального вывода.

## MCP-сервер

Для установленных клиентов предпочитайте общий локальный HTTP-сервис, чтобы Cursor, Codex, Kimi и другие клиенты переиспользовали один процесс `fighorse` вместо порождения stdio-подпроцессов:

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

Установите и запустите локальный сервис через явный путь сервиса, когда возможно:

```bash
fighorse install --default --mode service --clients cursor,codex,kimi --apply
```

Для разработки можно также запускать напрямую через `fighorse mcp serve --transport sse --host 127.0.0.1 --port 9449`.

HTTP-эндпоинты:

```text
http://127.0.0.1:9449/mcp
http://127.0.0.1:9449/sse
http://127.0.0.1:9449/manifest
http://127.0.0.1:9449/health
```

Сервис по умолчанию привязывается к `127.0.0.1` и использует singleton-лок в `~/.fighorse/runtime`. Используйте `--host` явно только когда намерены выставить сервис за пределы localhost. Используйте stdio только для клиентов, которые не могут подключиться к локальному HTTP-эндпоинту.

Примечание для мейнтейнеров: Streamable HTTP-клиенты, такие как Codex, могут инициализировать свежее MCP-соединение каждый раз при запуске. Поэтому эндпоинт `/mcp` должен терпеть повторяющиеся рукопожатия. В stateless-режиме создавайте и закрывайте свежий транспорт/сервер на каждый запрос. Переиспользование уже инициализированного `StreamableHTTPServerTransport` может привести к тому, что первое рукопожатие пройдет, а последующие провалятся с `500 text/plain` вместо MCP JSON/SSE-ответа.

Обычные CLI-команды, такие как `fighorse file get`, `fighorse design package` и `fighorse image export`, являются одноразовыми процессами. Им разрешается запускаться каждый раз, они не запускают MCP-сервис, не привязывают порты, не берут MCP singleton-лок и должны выходить после записи вывода. Figma HTTP-вызовы и загрузки изображений используют `FIGHORSE_HTTP_TIMEOUT_MS` с дефолтом `120000`, а `SIGINT`/`SIGTERM` прерывают выполняющиеся запросы перед выходом. `fighorse install --default --apply` по умолчанию настраивает только CLI; используйте `fighorse install --default --mode service --apply` или `fighorse install service --apply` только когда явно хотите, чтобы fighorse настроил или запустил долгоживущий MCP-сервис.

## Режимы безопасности

Инструменты записи Figma скрыты, если не включены:

```bash
FIGHORSE_MCP_MODE=write fighorse mcp serve --transport sse
```

Локальный экспорт файлов контролируется отдельно:

```bash
FIGHORSE_MCP_LOCAL_WRITE=allow fighorse mcp serve --transport sse
```

Даже при включенной локальной записи пути экспорта валидируются и должны оставаться в пределах `./.fighorse/exports`, `./assets/fighorse` или `~/.fighorse/exports`.

## Хранилище опыта

fighorse хранит переиспользуемые уроки в виде append-only JSONL. Это позволяет будущим AI-запускам учиться на предыдущем визуальном дебаггинге без изменения бинарника fighorse.

Пути по умолчанию:

- Домашний каталог: `~/.fighorse`.
- Глобальный опыт: `~/.fighorse/experience/global.jsonl`.
- Проектный опыт: `./.fighorse/experience.jsonl` после `fighorse install project`.
- Точное переопределение: `FIGHORSE_EXPERIENCE_PATH`.
- Переопределение области: `FIGHORSE_EXPERIENCE_SCOPE=global|project|merged`.

Команды:

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

## Типовые рабочие процессы

Используйте fighorse перед реализацией экрана Figma:

```bash
fighorse discover --format json
fighorse experience summary --platform web-react --asset-format svg
fighorse design package "<figma-url>" --platform web-react --asset-format svg --output ./.fighorse/exports/package.json
fighorse visual audit "<figma-url>" --screenshot ./.fighorse/exports/app-screen.png --platform web-react --asset-format svg
fighorse project playbook --platform web-react --asset-format svg
```

Синхронизация токенов:

```bash
fighorse file tokens <design_system_key> --format css --output src/styles/tokens.css
```

Пакетный экспорт фреймов:

```bash
IDS=$(fighorse file get <file_key> --depth 2 | jq -r '.. | objects | select(.type == "FRAME") | .id' | paste -sd, -)
fighorse image export <file_key> --ids "$IDS" --dir ./.fighorse/exports --manifest
```

## Устранение неполадок

- Первый запуск неясен: запустите `fighorse quickstart "<figma-frame-url>"` и следуйте `next_steps`.
- `doctor.auth.has_token` равен false: запустите `fighorse auth login --token <FIGMA_TOKEN>` или `fighorse install auth --apply`.
- `doctor.checks` сообщает, что MCP-сервис не запущен: игнорируйте для CLI-only работы или запустите `fighorse install --default --mode service --clients cursor,codex,kimi --apply` для AI-клиентов.
- Codex/Cursor сообщает `text/plain` или повторяющиеся initialize-ошибки: перезапустите fighorse-сервис после обновления; `/mcp` должен поддерживать повторяющиеся Streamable HTTP-рукопожатия.
- `smoke.ok` равен false, но метаданные файла существуют: следуйте `diagnostics.warnings`; часто выбранная цель слишком широка или отсутствует платформа/формат ассетов.
- MCP-инструмент экспорта сообщает, что локальная запись отключена: установите `FIGHORSE_MCP_LOCAL_WRITE=allow` в окружении MCP-сервера.
- Путь экспорта отклонен: используйте `./.fighorse/exports`, `./assets/fighorse` или `~/.fighorse/exports`.
- AI реализует целую страницу пользовательского потока: сузьте Figma URL до конкретного узла Frame/Screen перед реализацией.
