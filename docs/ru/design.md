# Архитектура fighorse

`fighorse` — это Rust CLI и MCP-сервер для преобразования данных Figma REST API в контекст уровня реализации для людей и AI-агентов. Это public-first инфраструктура: достаточно простая, чтобы новый пользователь быстро получил первый успешный design package, и достаточно глубокая, чтобы команды и AI-инструменты могли выстраивать воспроизводимые визуальные рабочие процессы.

## Цели продукта

Проект существует потому, что распространенные пути от Figma к AI каждый раз упускают что-то важное:

- Официальный Figma MCP мощный, но black-box поведение усложняет отладку, а будущие ограничения ценообразования или планов могут повлиять на доступность.
- Prompting только со скриншотами теряет точную раскладку, цвета, типографику, constraints и метаданные компонентов.
- Сырой Figma JSON слишком большой и шумный для контекстных окон LLM.
- MCP-only инструменты сообщества полезны внутри IDE, но слабы для скриптов, CI, воспроизводимой отладки и non-MCP агентов.

Цель fighorse — предоставить white-box data pipeline с тремя прогрессивными уровнями:

- Простой первый запуск: установка, токен, ссылка на конкретный фрейм, quickstart-проверка, design package.
- Глубокий второй запуск: asset-манифесты, точный REST dispatch, visual audit, project playbook.
- Долгосрочное обучение: локальные записи опыта, которые улучшают будущие AI-запуски без скрытой памяти.

Pipeline предоставляет:

- Точные факты Figma: структура нод, размеры, стили, раскладка, изображения, токены, метаданные.
- AI-ready контекст: компактный, с учетом бюджета токенов и явными предположениями.
- Визуальные референсы: URL скриншотов/рендеров и локальные экспортированные ассеты с манифестами.
- Нейтральный к инструментам доступ: сначала CLI, MCP как адаптер, устанавливаемые skills/rules для AI-клиентов.
- Память обратной связи: локальное хранилище опыта, чтобы уроки визуальной отладки использовались повторно.

## Основной принцип: CLI — ядро, MCP — оболочка

CLI — это основная граница продукта. MCP предоставляет те же возможности AI-инструментам, но остается тонким слоем.

```text
Figma REST API
  -> API-модули + OpenAPI operation registry
  -> продуктовый слой: compact/filter/tokens/assets/design-package/visual-audit/playbook
  -> CLI-вывод, файлы, манифесты
  -> MCP tools/resources/prompts, AI-клиенты, скрипты, CI
```

Это делает систему:

- Public-first: пользователи могут успешно работать через CLI до изучения MCP.
- Инспектируемой: разработчики могут запустить ту же команду, что вызывает AI-инструмент.
- Скриптуемой: shell, CI и кастомные агенты могут использовать бинарник напрямую.
- Транспортно-нейтральной: общий Streamable HTTP, явный standard stdio MCP и CLI разделяют бизнес-поведение.
- Проще в тестировании: чистые трансформации и API-обертки остаются отделимыми от конфигурации клиента.

## Многоуровневая архитектура

```text
L4 Адаптеры
  mcp serve, resources/prompts, install client, install service, generated skills/rules

L3 Сгенерированный контекст
  design package, visual audit, project playbook, markdown, tokens, tree, schema, manifests

L2 Трансформации
  compact, filter, diff, diagnostics, experience matching, API coverage reports

L1 Figma API и ассеты
  OpenAPI registry, operation dispatcher, files, nodes, images, comments, components, styles, variables, webhooks, downloads
```

L2 и L3 — основные дифференциаторы. fighorse не просто оборачивает REST endpoint'ы; он переформатирует данные Figma в форму, которую AI-агент может использовать, не тонув в шуме.

## Полное покрытие REST

fighorse поддерживает явный OpenAPI operation registry для публичного снапшота Figma REST. Реестр в настоящее время отслеживает 48 операций и используется:

- API-обертками в `src/fighorse/api`.
- Универсальным CLI dispatch: `fighorse figma api <operationId> --params '{...}'`.
- Сгенерированными официальными MCP tools: `figma_<operation_id_in_snake_case>`.
- Discovery и coverage reports: `fighorse figma-api coverage`.
- Контрактными тестами, предотвращающими пропущенные или устаревшие endpoint'ы.

Универсальный официальный слой отделен от продуктовых инструментов. Низкоуровневые REST tools сохраняют семантику Figma; продуктовые инструменты, такие как `get_design_package`, `visual_audit` и `get_project_playbook`, добавляют AI workflow guidance поверх.

## Design Package

`fighorse design package` и MCP `get_design_package` — предпочтительный high-level интерфейс для реализации дизайна. Пакет объединяет:

- Распарсенный Figma URL и выбранную целевую ноду.
- Компактный структурный контекст.
- Токены и подсказки по реализации.
- Figma render-референсы и опциональные URL ассетов.
- Предположения о платформе и формате ассетов.
- Кандидаты на screen и component.
- План экспорта с примерами CLI и MCP-вызовов.
- Локальный изученный опыт.
- Confidence токенов, диагностика отсутствующих шрифтов и чеклист рисков реализации.
- Диагностику и предупреждения о следующих шагах.

Пакет разработан так, чтобы быть одновременно машиночитаемым и легким для инспекции. Он должен сообщать AI не только что реализовывать, но и чего не хватает до того, как реализация станет безопасной.

Важная диагностика включает:

- Отсутствующий `platform`.
- Отсутствующий `asset_format`.
- Неподдерживаемый render-формат для рендеринга нод Figma.
- Целевая нода — широкий `CANVAS` или страница user-flow.
- Усечение контекста из-за бюджета токенов.
- Отсутствующие скриншоты, токены или image fills.

`visual_audit` и `project playbook` расширяют пакет до полного feedback loop. `visual_audit` превращает Figma URL плюс опциональный скриншот приложения в структурированный чеклист сравнения и предложения по опыту. `project playbook` объединяет AI-контракт, output policy, покрытие API и локальные уроки в reusable project instructions.

## Self-Discovery Contract

AI-клиенты не должны полагаться на устаревший prompt text. Они должны вызывать:

- CLI: `fighorse discover --format json`
- MCP: `discover_fighorse`

Discovery manifest описывает:

- Доступные CLI и MCP capabilities.
- Покрытие REST и сравнение с официальным MCP.
- Рекомендуемый workflow репликации дизайна.
- Safety defaults.
- Требования к локальной записи.
- Поведение experience-store.
- Текущий AI-контракт.
- Подсказки по установке и конфигурации клиента.
- MCP resources и prompts для клиентов, которые их поддерживают.

`doctor` дополняет discovery runtime-статусом: информация о runtime, статус auth, home directory, MCP mode, local-write mode и готовность experience-store.

## Модель самообучения

fighorse хранит reusable lessons в append-only JSONL-хранилищах:

- Глобальное: `~/.fighorse/experience/global.jsonl`.
- Проектное: `./.fighorse/experience.jsonl` после `fighorse install project`.
- Точное переопределение: `FIGHORSE_EXPERIENCE_PATH`.
- Переопределение scope: `FIGHORSE_EXPERIENCE_SCOPE=global|project|merged`.

Это намеренно локально и прозрачно. Цель не в автономной скрытой памяти; цель — в project/user-owned log практических уроков репликации дизайна.

Хорошие записи описывают reusable patterns:

- Перекрытие раскладки, вызванное маппингом повторяющихся siblings в stacking-контейнер.
- Компактная типографика компонентов, требующая прямой инспекции вместо глобального скейлинга.
- Несоответствие status bar или safe-area устройства.
- Ограничения формата ассетов или доступности шрифтов.

Записи не должны содержать секретов, приватных абсолютных путей или one-off project details.

## Границы безопасности

fighorse разделяет три домена безопасности:

- Доступ на чтение Figma: требует токена, это нормальный режим работы.
- Доступ на запись Figma: отключен, пока не задан `FIGHORSE_MCP_MODE=write`.
- Локальная запись в файловую систему: отключена в MCP, пока не задан `FIGHORSE_MCP_LOCAL_WRITE=allow`.

Локальные экспорты файлов все еще ограничены approved roots:

- `./.fighorse/exports`
- `./assets/fighorse`
- `~/.fighorse/exports`

Это разделение важно, потому что загрузка локальных скриншотов или image fills гораздо менее чувствительна, чем мутация Figma, но все же требует валидации пути. Пользователь явно контролирует оба домена.

Default MCP adapter — official Rust `rmcp` 2.2 `StreamableHttpService` с `LocalSessionManager`: независимые stateful sessions, проверка Host и Origin до dispatch, JSON или event-stream response по standard Streamable HTTP и graceful shutdown по SIGINT/SIGTERM. Event stream на `/mcp` не является legacy SSE transport; `/sse` и `/messages` не обслуживаются, а `--transport sse` завершается ошибкой. Explicit compatibility использует standard rmcp stdio без private framing protocol.

## Дизайн установки

`fighorse install` по умолчанию генерирует reviewable артефакты и мутирует user-level конфиг только с `--apply`. Инсталлятор нацелен на:

- Настройку home/config в `~/.fighorse`.
- Установку бинарника в `~/.fighorse/bin/fighorse`.
- Хранение auth в `~/.fighorse/config.json`.
- MCP client snippets для Cursor, Codex, Kimi, Claude, opencode и generic агентов.
- User-level skills/rules для AI-клиентов.
- Пользовательские сервисы macOS launchd и Linux systemd для Streamable HTTP MCP.

Когда доступны нативные команды клиента, инсталлятор предпочитает их. Иначе он пишет стандартные пользовательские конфигурационные файлы с бэкапами.

`fighorse install --default --apply` по умолчанию работает в CLI-only режиме. Команда для четырёх клиентов: `fighorse install --default --mode service --clients cursor,codex,kimi,claude --apply`; только Claude: `fighorse install client --client claude --apply`.

Payload: Cursor `{"url":"http://127.0.0.1:9449/mcp"}`, Kimi `{"transport":"http","url":"http://127.0.0.1:9449/mcp"}`, Claude `{"type":"http","url":"http://127.0.0.1:9449/mcp"}`, Codex — `[mcp_servers.fighorse]` с тем же URL.

Транзакция: `preflight -> backup -> binary -> service -> health_ready -> clients -> skills -> verified`; client config не записывается до `/health` и реальных `initialize`/`tools/list`. Manifest хранит hash, backup, порядок и `desired_absent: true`; rollback восстанавливает managed files и service state в обратном порядке, а custom skill conflict сохраняется с deterministic backup.

Три canonical-цели: `~/.agents/skills/fighorse/SKILL.md` (Cursor/Kimi/Codex), `~/.claude/skills/fighorse/SKILL.md` (Claude), `~/.cursor/rules/fighorse.mdc` (Cursor). Fresh service/stdio deny local write; migration сохраняет только существующий explicit allow.

## Позиция в экосистеме

fighorse заимствует уроки у существующих инструментов, выбирая другую границу.

Официальный Figma MCP — prescriptive и глубоко интегрированный. Он может предоставлять Code Connect, Code to Canvas и design-system search, но его поведение непрозрачно и привязано к product surface Figma.

Framelink-style MCP — descriptive и легковесный. Он дает AI факты о раскладке и стилях вместо сгенерированной кодовой структуры. Это избегает "отравления" контекста решением о фреймворке, которое текущая кодовая база может не захотеть.

fighorse по умолчанию идет по descriptive пути: факты вместо сгенерированного кода. Он все еще может предоставлять более богатые Figma-метаданные и write-capable endpoint'ы при явном включении, но основной workflow — "точный контекст на входе, project-native реализация на выходе".

Product surfaces, доступные только через official API и не входящие в public REST OpenAPI, помечаются как unsupported by public REST вместо того, чтобы быть тихо аппроксимированными. Примеры включают нативную canvas mutation, Code to Canvas, automatic Code Connect mapping discovery, Make resources и FigJam generation. fighorse может предлагать open alternatives, такие как user-maintained component maps или generic resource ingestion, но не должен притворяться, что реализует приватные Figma product API.

## Достоверность данных

Данные Figma REST API в целом specification-faithful для фактов реализации:

- Геометрия, размеры, bounds и координаты.
- Цвета, эффекты, обводки, fills, corner radii.
- Текстовые символы и метаданные стилей.
- Auto Layout properties.
- Отношения component и instance.
- Render URL и image fills.

Pixel-perfect рендеринг в браузере или на мобильном устройстве все еще требует feedback loop, потому что рендеринг-движки, доступность шрифтов, антиалиасинг, blend modes и системный UI отличаются от Figma. Поэтому fighorse комбинирует структурированный JSON со скриншотами и настаивает на визуальной верификации вместо заявлений о first-pass perfection.

## Уроки из полевого использования

Реальный Android Compose-прототип выявил несколько практических требований:

- `design package` должен нормализовывать Figma URL и node ids.
- `smoke` и `diagnostics.status=ready` — полезные readiness signals.
- Export manifests надежнее terminal logs для downstream скриптов.
- Безопасные имена файлов, такие как `376_12995.png`, лучше работают в build-системах приложений.
- Широкие flow-ноды должны сужаться до конкретных фреймов перед реализацией.
- Повторяющиеся строки нуждаются в list/linear контейнерах; generic stack containers вызывают overlap.
- Компактные message cards и полноразмерные chat cards могут требовать разной типографики.
- Мобильные экраны нуждаются в scroll-safe раскладках.
- Доступность шрифтов и поведение status-bar/safe-area должны обрабатываться явно.
- AI-инструментам нужны выбор platform и asset-format перед реализацией.

Эти уроки напрямую сформировали контракты design package, export manifest, local-write и experience-store.

## Non-Goals

fighorse не стремится быть универсальным генератором кода. Код должен производиться AI-агентом с использованием существующих паттернов целевого репозитория.

Он также не стремится заменить собственные product-интеграции Figma. Official MCP остается подходящим для Code Connect, Code to Canvas, FigJam generation или native Figma mutation workflows.

Долгосрочная продуктовая граница — это pipeline контекста и ассетов: fetch, compact, explain, export, verify и learn.

## Стратегия верификации

Проект должен оставаться верифицируемым без реального Figma-токена:

- Unit и integration tests покрывают парсинг аргументов, compacting, URL, валидацию путей, OpenAPI, MCP routing, official HTTP/standard stdio, discovery manifest, install transaction, design-package diagnostics и docs consistency.
- Интеграционные тесты, обращающиеся к реальному Figma API, опциональны.
- Документация и сгенерированные install артефакты должны отражать текущее поведение CLI.

Ведущее правило поддержки — консистентность между CLI, MCP schemas, installer output, discovery manifest, skills/rules и формальной документацией.
