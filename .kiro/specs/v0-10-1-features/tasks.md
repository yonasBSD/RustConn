# Задачі імплементації: RustConn v0.10.1

## Фаза 1: Per-connection Terminal Theming (P2)

- [x] 1. Реалізувати модель Terminal Theming у rustconn-core
  - [x] 1.1 Створити `ConnectionThemeOverride` у connection.rs
    - Додати структуру `ConnectionThemeOverride` з полями `background`, `foreground`, `cursor` (усі `Option<String>`)
    - Реалізувати `validate()` — regex `^#[0-9a-fA-F]{6}([0-9a-fA-F]{2})?$`
    - Реалізувати `is_empty()` — true якщо всі поля None
    - Додати `#[serde(default, skip_serializing_if = "Option::is_none")]` для кожного поля
    - _Вимоги: 1.1, 1.6_

  - [x] 1.2 Додати поле `theme_override` до `Connection`
    - Додати `pub theme_override: Option<ConnectionThemeOverride>` з `#[serde(default, skip_serializing_if = "Option::is_none")]`
    - Оновити `Connection::new()` — `theme_override: None`
    - _Вимоги: 1.1, 1.5_

  - [x] 1.3 Написати property-тести для Terminal Theming
    - **Proptest 1:** Будь-який валідний hex `#[0-9a-fA-F]{6}` проходить валідацію
    - **Proptest 2:** Невалідний рядок (без #, неправильна довжина, невалідні символи) не проходить
    - **Proptest 3:** Serde round-trip — serialize → deserialize = ідентичний результат
    - Створити файл `rustconn-core/tests/properties/terminal_theme_override_tests.rs`
    - Зареєструвати модуль у `rustconn-core/tests/properties/mod.rs`
    - _Вимоги: 1.1, 1.6, 11.2_

- [x] 2. Контрольна точка — переконатися, що всі тести проходять
  - Виконати `cargo test -p rustconn-core --test property_tests`
  - Виконати `cargo clippy -p rustconn-core -- -D warnings`

## Фаза 2: CSV Import/Export (P4)

- [x] 3. Реалізувати CSV Import/Export у rustconn-core
  - [x] 3.1 Створити `rustconn-core/src/import/csv.rs`
    - Створити `CsvImporter` struct
    - Створити `CsvColumnMapping` з полями: `name_col`, `host_col`, `port_col`, `protocol_col`, `username_col`, `group_col`, `tags_col`, `description_col`
    - Створити `CsvParseOptions` з полями: `delimiter` (u8), `has_header` (bool), `mapping` (Option)
    - Реалізувати `ImportSource` trait для `CsvImporter`
    - Реалізувати автоматичний маппінг заголовків (case-insensitive matching відомих імен колонок)
    - Реалізувати парсинг тегів (розділювач `;`) та груп (шлях через `/`)
    - Реалізувати case-insensitive matching протоколів до `ProtocolType`
    - Зареєструвати: `mod csv; pub use csv::CsvImporter;` у `import/mod.rs`
    - Додати `csv = "1"` до `rustconn-core/Cargo.toml`
    - _Вимоги: 2.1, 2.2, 2.3, 2.4, 2.5, 2.6, 2.7_

  - [x] 3.2 Створити `rustconn-core/src/export/csv.rs`
    - Створити `CsvExporter` struct
    - Створити `CsvExportOptions` з полями: `delimiter` (u8), `fields` (Vec<CsvExportField>)
    - Створити `CsvExportField` enum: Name, Host, Port, Protocol, Username, GroupName, Tags, Description
    - Реалізувати `ExportTarget` trait для `CsvExporter`
    - Додати `Csv` варіант до `ExportFormat` enum з `display_name()` "CSV", `file_extension()` "csv"
    - Оновити `ExportFormat::all()` щоб включити `Csv`
    - Зареєструвати: `pub mod csv;` у `export/mod.rs`
    - _Вимоги: 3.1, 3.2, 3.3, 3.4, 3.5, 3.6, 3.7_

  - [x] 3.3 Написати property-тести для CSV
    - **Proptest 4:** Round-trip — export → parse → еквівалентні з'єднання
    - **Proptest 5:** Рядки з комами, лапками, переносами коректно квотуються (RFC 4180)
    - **Proptest 6:** Пропущені обов'язкові поля (name, host) → SkippedEntry
    - Створити файл `rustconn-core/tests/properties/csv_tests.rs`
    - Зареєструвати модуль у `rustconn-core/tests/properties/mod.rs`
    - _Вимоги: 2.10, 3.8, 11.1_

- [x] 4. Контрольна точка — переконатися, що всі тести проходять
  - Виконати `cargo test -p rustconn-core --test property_tests`
  - Виконати `cargo clippy -p rustconn-core -- -D warnings`

## Фаза 3: MOSH Protocol (P6)

- [x] 5. Реалізувати MOSH протокол у rustconn-core
  - [x] 5.1 Додати MOSH до моделі протоколів
    - Створити `MoshPredictMode` enum (Adaptive, Always, Never) з `#[default] Adaptive` у `protocol.rs`
    - Створити `MoshConfig` struct з полями: `ssh_port`, `port_range`, `server_binary`, `predict_mode`, `custom_args`
    - Додати `Mosh` варіант до `ProtocolType` enum
    - Додати `Mosh(MoshConfig)` варіант до `ProtocolConfig` enum
    - Оновити `ProtocolConfig::protocol_type()` для Mosh
    - Оновити `Connection::default_port()` — `ProtocolType::Mosh => 22`
    - Оновити всі `match` на `ProtocolType` та `ProtocolConfig` у кодовій базі
    - _Вимоги: 4.1, 4.2, 4.8_

  - [x] 5.2 Створити protocol handler `rustconn-core/src/protocol/mosh.rs`
    - Створити `MoshProtocol` struct
    - Реалізувати `Protocol` trait: `protocol_id` "mosh", `display_name` "MOSH", `default_port` 22
    - Реалізувати `capabilities()` → `ProtocolCapabilities::terminal()`
    - Реалізувати `build_command()` — побудова команди `mosh` з усіма опціями
    - Додати `detect_mosh()` у `detection.rs` — `which("mosh")`
    - Зареєструвати: `pub mod mosh;` у `protocol/mod.rs`
    - _Вимоги: 4.3, 4.4, 4.5_

  - [x] 5.3 Написати property-тести для MOSH
    - **Proptest 7:** Serde round-trip для MoshConfig
    - **Proptest 8:** `build_command()` з різними комбінаціями опцій генерує валідну команду
    - **Proptest 9:** Порожній host → помилка
    - Створити файл `rustconn-core/tests/properties/mosh_tests.rs`
    - Зареєструвати модуль у `rustconn-core/tests/properties/mod.rs`
    - _Вимоги: 4.7, 11.3_

- [x] 6. Контрольна точка — переконатися, що всі тести проходять
  - Виконати `cargo test -p rustconn-core --test property_tests`
  - Виконати `cargo clippy -p rustconn-core -- -D warnings`

## Фаза 4: Dynamic Credential Resolution — Script (P8)

- [x] 7. Реалізувати Script password source у rustconn-core
  - [x] 7.1 Додати `Script` варіант до `PasswordSource`
    - Додати `Script(String)` до `PasswordSource` enum у `connection.rs`
    - Переконатися, що serde серіалізація/десеріалізація працює
    - Додати `shell-words = "1"` до `rustconn-core/Cargo.toml`
    - _Вимоги: 5.1, 5.8_

  - [x] 7.2 Створити `ScriptResolver` у `rustconn-core/src/secret/script_resolver.rs`
    - Реалізувати розбиття команди через `shell_words::split()`
    - Реалізувати виконання через `tokio::process::Command` (без shell)
    - Реалізувати timeout 30 секунд через `tokio::time::timeout`
    - stdout → trim → `SecretString`; очистити буфер після обгортання
    - Non-zero exit → `SecretError::RetrieveFailed` з stderr
    - Timeout → `SecretError::RetrieveFailed` з повідомленням
    - Зареєструвати: `pub mod script_resolver;` у `secret/mod.rs`
    - _Вимоги: 5.2, 5.3, 5.4, 5.5, 5.6, 5.9_

  - [x] 7.3 Інтегрувати ScriptResolver у credential resolution chain
    - Додати гілку для `PasswordSource::Script` у існуючий credential resolver
    - _Вимоги: 5.10_

  - [x] 7.4 Написати property-тести для Script credentials
    - **Proptest 10:** Serde round-trip для `PasswordSource::Script(command)`
    - **Proptest 11:** Довільний command string зберігається через serialize/deserialize
    - Створити файл `rustconn-core/tests/properties/script_resolver_tests.rs`
    - Зареєструвати модуль у `rustconn-core/tests/properties/mod.rs`
    - _Вимоги: 5.8, 11.4, 11.8_

- [x] 8. Контрольна точка — переконатися, що всі тести проходять
  - Виконати `cargo test -p rustconn-core --test property_tests`
  - Виконати `cargo clippy -p rustconn-core -- -D warnings`

## Фаза 5: Session Recording (P1)

- [x] 9. Реалізувати Session Recording у rustconn-core
  - [x] 9.1 Створити `rustconn-core/src/session/recording.rs`
    - Створити `SessionRecorder` struct з полями: data_file, timing_file, last_timestamp, sanitize
    - Реалізувати `SessionRecorder::new(data_path, timing_path, sanitize)` — створення файлів
    - Реалізувати `write_chunk(&[u8])` — запис з sanitization та timing
    - Створити `RecordingReader` struct для читання записів
    - Реалізувати `RecordingReader::next_chunk()` → `Option<(Duration, Vec<u8>)>`
    - Формат timing: `{delay_seconds} {byte_count}\n` (сумісний з `scriptreplay`)
    - _Вимоги: 6.1, 6.2, 6.10_

  - [x] 9.2 Додати поле `session_recording_enabled` до `Connection`
    - Додати `pub session_recording_enabled: bool` з `#[serde(default)]`
    - Оновити `Connection::new()` — `session_recording_enabled: false`
    - _Вимоги: 6.4_

  - [x] 9.3 Реалізувати шлях та іменування файлів
    - Шлях: `$XDG_DATA_HOME/rustconn/recordings/`
    - Іменування: `{connection_name}_{timestamp}.{data|timing}` з sanitized name
    - Обробка помилок: якщо директорія не writable → log error, disable recording
    - _Вимоги: 6.7, 6.8, 6.9_

  - [x] 9.4 Реалізувати sanitization записів
    - Використати існуючі log sanitization patterns (паролі, API keys, tokens)
    - _Вимоги: 6.10_

  - [x] 9.5 Написати property-тести для Session Recording
    - **Proptest 12:** Round-trip — write timing+data → read timing+data → ідентичні chunks
    - **Proptest 13:** Довільні байти коректно записуються та читаються
    - Створити файл `rustconn-core/tests/properties/recording_tests.rs`
    - Зареєструвати модуль у `rustconn-core/tests/properties/mod.rs`
    - _Вимоги: 6.11, 11.7_

- [x] 10. Контрольна точка — переконатися, що всі тести проходять
  - Виконати `cargo test -p rustconn-core --test property_tests`
  - Виконати `cargo clippy -p rustconn-core -- -D warnings`

## Фаза 6: Text Highlighting Rules (P3)

- [x] 11. Реалізувати Highlight Rules у rustconn-core
  - [x] 11.1 Створити модель `rustconn-core/src/models/highlight.rs`
    - Створити `HighlightRule` struct з полями: `id` (Uuid), `name` (String), `pattern` (String), `foreground_color` (Option<String>), `background_color` (Option<String>), `enabled` (bool)
    - Реалізувати `validate_pattern()` — перевірка regex через `regex::Regex::new()`
    - Додати Serialize/Deserialize derives
    - Зареєструвати: `pub mod highlight;` у `models/mod.rs`
    - _Вимоги: 7.1, 7.7, 7.8_

  - [x] 11.2 Створити engine `rustconn-core/src/highlight.rs`
    - Створити `CompiledHighlightRules` struct
    - Реалізувати `compile(global_rules, per_conn_rules)` — per-connection пріоритет
    - Реалізувати `find_matches(line)` → `Vec<HighlightMatch>` з позиціями та кольорами
    - Невалідний regex → skip rule + log warning
    - Built-in defaults: ERROR (red), WARNING (yellow), CRITICAL/FATAL (red bg)
    - Зареєструвати: `pub mod highlight;` у `lib.rs`
    - _Вимоги: 7.2, 7.3, 7.4, 7.7, 7.9_

  - [x] 11.3 Додати `highlight_rules` до `Connection`
    - Додати `pub highlight_rules: Vec<HighlightRule>` з `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
    - _Вимоги: 7.5_

  - [x] 11.4 Написати property-тести для Highlighting
    - **Proptest 14:** Валідний regex проходить `validate_pattern()`, невалідний — ні
    - **Proptest 15:** Serde round-trip для HighlightRule
    - **Proptest 16:** Matching positions коректні для відомих патернів
    - Створити файл `rustconn-core/tests/properties/highlight_tests.rs`
    - Зареєструвати модуль у `rustconn-core/tests/properties/mod.rs`
    - _Вимоги: 7.8, 11.6_

- [x] 12. Контрольна точка — переконатися, що всі тести проходять
  - Виконати `cargo test -p rustconn-core --test property_tests`
  - Виконати `cargo clippy -p rustconn-core -- -D warnings`

## Фаза 7: Smart Folders (P5)

- [x] 13. Реалізувати Smart Folders у rustconn-core
  - [x] 13.1 Створити модель `rustconn-core/src/models/smart_folder.rs`
    - Створити `SmartFolder` struct з полями: `id` (Uuid), `name` (String), `filter_protocol` (Option<ProtocolType>), `filter_tags` (Vec<String>), `filter_host_pattern` (Option<String>), `filter_group_id` (Option<Uuid>), `sort_order` (i32)
    - Додати Serialize/Deserialize derives з відповідними serde атрибутами
    - Зареєструвати: `pub mod smart_folder;` у `models/mod.rs`
    - Додати `glob = "0.3"` до `rustconn-core/Cargo.toml`
    - _Вимоги: 9.1, 9.8_

  - [x] 13.2 Створити `SmartFolderManager` у `rustconn-core/src/smart_folder.rs`
    - Реалізувати CRUD: `new()`, `add()`, `remove()`, `get()`, `list()`
    - Реалізувати `evaluate(folder, connections)` — AND логіка для всіх фільтрів
    - Порожній фільтр → порожній результат
    - Glob matching для `filter_host_pattern` через крейт `glob`
    - Зареєструвати: `pub mod smart_folder;` у `lib.rs`
    - _Вимоги: 9.2, 9.3, 9.7_

  - [x] 13.3 Написати property-тести для Smart Folders
    - **Proptest 17:** З'єднання, що відповідає всім критеріям → присутнє у результаті
    - **Proptest 18:** З'єднання, що не відповідає хоча б одному критерію → відсутнє
    - **Proptest 19:** Serde round-trip для SmartFolder
    - **Proptest 20:** Порожній фільтр → порожній результат
    - Створити файл `rustconn-core/tests/properties/smart_folder_tests.rs`
    - Зареєструвати модуль у `rustconn-core/tests/properties/mod.rs`
    - _Вимоги: 9.2, 9.3, 9.8, 11.5_

- [x] 14. Контрольна точка — переконатися, що всі тести проходять
  - Виконати `cargo test -p rustconn-core --test property_tests`
  - Виконати `cargo clippy -p rustconn-core -- -D warnings`

## Фаза 8: GUI — Terminal Theming + MOSH + Script Credentials (Фаза 1 features)

- [x] 15. Реалізувати GUI для Terminal Theming
  - [x] 15.1 Додати секцію "Terminal Theme" в Advanced tab
    - Додати 3 ColorDialogButton (background, foreground, cursor) у `rustconn/src/dialogs/connection/advanced_tab.rs`
    - Додати кнопку "Reset" для очищення override
    - Додати preview міні-прямокутник з обраними кольорами
    - _Вимоги: 1.2, 1.7_

  - [x] 15.2 Інтегрувати theme override у VTE widget
    - При створенні VTE — конвертувати hex у RGBA та застосувати set_color_background/foreground/cursor
    - Якщо theme_override відсутній — використати глобальну тему
    - _Вимоги: 1.3, 1.4_

- [x] 16. Реалізувати GUI для MOSH
  - [x] 16.1 Створити MOSH tab у connection dialog
    - Додати SSH Port (SpinButton), Port Range (Entry), Predict Mode (ComboRow), Server Binary (Entry)
    - Показувати tab тільки коли обрано протокол MOSH
    - _Вимоги: 4.6_

- [x] 17. Реалізувати GUI для Script Credentials
  - [x] 17.1 Додати Script option до auth tab
    - Додати Entry для команди з placeholder прикладом
    - Додати кнопку "Test Script" для перевірки
    - Показувати поле тільки коли обрано PasswordSource::Script
    - _Вимоги: 5.7_

- [x] 18. Контрольна точка — переконатися, що GUI компілюється
  - Виконати `cargo clippy -p rustconn -- -D warnings`

## Фаза 9: GUI — CSV Import/Export

- [x] 19. Реалізувати GUI для CSV Import/Export
  - [x] 19.1 Додати CSV до Import dialog
    - Додати CSV format option у import dialog
    - Реалізувати column mapping preview перед імпортом
    - Додати вибір delimiter (comma, semicolon, tab)
    - _Вимоги: 2.8, 2.9_

  - [x] 19.2 Додати CSV до Export dialog
    - Додати CSV format option у export dialog
    - Додати вибір полів для експорту
    - Додати вибір delimiter
    - _Вимоги: 3.6_

- [x] 20. Контрольна точка — переконатися, що GUI компілюється
  - Виконати `cargo clippy -p rustconn -- -D warnings`

## Фаза 10: GUI — Session Recording + Highlighting (Фаза 2 features)

- [x] 21. Реалізувати GUI для Session Recording
  - [x] 21.1 Додати Recording toggle в Advanced tab
    - Додати "Record Session" toggle у connection dialog Advanced tab
    - _Вимоги: 6.5_

  - [x] 21.2 Додати індикатор запису
    - Показувати "●REC" у заголовку вкладки терміналу під час запису
    - Підключити VTE commit callback до SessionRecorder write_chunk
    - _Вимоги: 6.3, 6.6_

- [x] 22. Реалізувати GUI для Highlighting
  - [x] 22.1 Створити Highlight Rules editor
    - Додати UI для управління per-connection rules у Connection Dialog (add, edit, delete, enable/disable)
    - Додати UI для глобальних rules у Settings dialog
    - _Вимоги: 7.6_

  - [x] 22.2 Інтегрувати highlighting у VTE
    - Застосувати VTE text attributes або overlay для підсвічування
    - _Вимоги: 7.3_

- [x] 23. Контрольна точка — переконатися, що GUI компілюється
  - Виконати `cargo clippy -p rustconn -- -D warnings`

## Фаза 11: GUI — Broadcast + Smart Folders

- [x] 24. Реалізувати Ad-hoc Broadcast
  - [x] 24.1 Створити BroadcastController у `rustconn/src/broadcast.rs`
    - Реалізувати BroadcastController struct з active, selected_terminals (HashSet)
    - Реалізувати методи: activate, deactivate, toggle_terminal, broadcast_input
    - _Вимоги: 8.1, 8.2_

  - [x] 24.2 Інтегрувати broadcast у toolbar та tabs
    - Додати toolbar toggle кнопку + keyboard shortcut
    - Додати чекбокси на вкладках терміналів при активному broadcast
    - Keystroke → feed_child до всіх обраних терміналів
    - Обробка закриття вкладки під час broadcast
    - _Вимоги: 8.1, 8.2, 8.3, 8.4, 8.5, 8.6, 8.7_

- [x] 25. Реалізувати GUI для Smart Folders
  - [x] 25.1 Додати Smart Folders секцію у sidebar
    - Окрема секція з іконкою, окремо від звичайних груп
    - Клік → список з'єднань (read-only, без drag-drop)
    - Контекстне меню: Edit / Delete
    - _Вимоги: 9.4, 9.5, 9.6_

  - [x] 25.2 Створити діалог створення/редагування Smart Folder
    - ComboRow (protocol), Entry (host pattern), TagEntry (tags), GroupPicker
    - _Вимоги: 9.1_

- [x] 26. Контрольна точка — переконатися, що GUI компілюється
  - Виконати `cargo clippy -p rustconn -- -D warnings`

## Фаза 12: CLI — CSV + Smart Folders

- [x] 27. Додати CLI subcommands
  - [x] 27.1 Додати CSV import/export до CLI
    - `rustconn-cli import --format csv --file <path>` з optional --delimiter та --mapping
    - `rustconn-cli export --format csv --file <path>` з optional --fields та --delimiter
    - _Вимоги: 2.8, 3.6_

  - [x] 27.2 Додати Smart Folders subcommands до CLI
    - `rustconn-cli smart-folders list` — список усіх smart folders
    - `rustconn-cli smart-folders show <name>` — з'єднання, що відповідають фільтру
    - `rustconn-cli smart-folders create --name <name> --protocol <proto> --host-pattern <pattern>`
    - `rustconn-cli smart-folders delete <name>`
    - _Вимоги: 9.9_

- [x] 28. Контрольна точка — переконатися, що CLI компілюється
  - Виконати `cargo clippy -p rustconn-cli -- -D warnings`

## Фаза 13: Локалізація (i18n)

- [x] 29. Створити скрипт локалізації
  - [x] 29.1 Створити `po/fill_i18n_0_10_1.py`
    - Слідувати патерну `fill_i18n_0_10_0.py`
    - Імпортувати parse_po_file, extract_msgid, extract_msgstr, rebuild_po_file з fill_translations.py
    - Заповнити TRANSLATIONS dict для всіх 15 мов: uk, de, fr, es, it, pl, cs, sk, da, sv, nl, pt, be, kk, uz
    - Покрити рядки: MOSH, CSV, Session Recording, Highlighting, Broadcast, Smart Folders, Script credentials, Terminal theming
    - _Вимоги: 10.1, 10.2, 10.3, 10.4_

  - [x] 29.2 Обгорнути всі нові рядки у i18n
    - Перевірити всі нові user-visible рядки у крейті rustconn
    - Обгорнути кожен рядок функцією i18n()
    - _Вимоги: 10.1_

## Фаза 14: Фінальна перевірка та Clippy Compliance

- [x] 30. Фінальна перевірка
  - [x] 30.1 Запустити Clippy на всіх крейтах
    - Виконати `cargo clippy -p rustconn-core --all-targets -- -D warnings`
    - Виконати `cargo clippy -p rustconn --all-targets -- -D warnings`
    - Виконати `cargo clippy -p rustconn-cli --all-targets -- -D warnings`
    - Виправити всі попередження
    - _Вимоги: 11.9_

  - [x] 30.2 Запустити повний набір тестів
    - Виконати `cargo test --workspace`
    - Переконатися, що всі тести проходять
    - _Вимоги: 11.9, 11.10_

  - [x] 30.3 Запустити property-тести з розширеними ітераціями
    - Виконати `cargo test -p rustconn-core --test property_tests -- --test-threads=1`
    - Перевірити, що всі 20 нових proptests проходять
    - _Вимоги: 11.1-11.8_

- [x] 31. Фінальна контрольна точка
  - Усі property-тести та unit-тести проходять
  - Clippy проходить без попереджень на всіх крейтах
  - Усі нові рядки обгорнуті i18n()
  - Зворотна сумісність збережена (serde default для нових полів)
