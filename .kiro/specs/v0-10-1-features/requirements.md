# Документ вимог: RustConn v0.10.1

## Вступ

Цей документ описує вимоги до нових функцій RustConn версії 0.10.1. Реліз охоплює 8 функціональних блоків, визначених за результатами конкурентного аналізу (docs/research/COMPETITIVE_ANALYSIS.md), а також наскрізні вимоги до локалізації (i18n) та автоматизованого тестування (proptest). Функції розділені на дві фази за пріоритетом.

**Фаза 1 (високий пріоритет):** Per-connection terminal theming (P2), CSV імпорт/експорт (P4), MOSH протокол (P6), Dynamic credential resolution (P8).

**Фаза 2 (середній пріоритет):** Session Recording (P1), Text highlighting rules (P3), Ad-hoc broadcast (P7), Smart Folders (P5).

## Глосарій

- **Connection_Dialog** — GTK4/libadwaita діалог створення та редагування з'єднання у крейті `rustconn`
- **Connection** — структура `Connection` у `rustconn-core/src/models/connection.rs`
- **ProtocolType** — перелік `ProtocolType` у `rustconn-core/src/models/protocol.rs`, що ідентифікує тип протоколу
- **ProtocolConfig** — перелік `ProtocolConfig` у `rustconn-core/src/models/protocol.rs`, що містить конфігурацію конкретного протоколу
- **PasswordSource** — перелік `PasswordSource` у `rustconn-core/src/models/connection.rs`, що визначає джерело пароля
- **Terminal_Theme** — структура у `rustconn-core`, що зберігає кольори терміналу (фон, текст, курсор) для конкретного з'єднання
- **VTE_Widget** — віджет терміналу VTE4 у крейті `rustconn`, що відображає термінальну сесію
- **CSV_Importer** — модуль у `rustconn-core/src/import/`, що парсить CSV-файли у з'єднання
- **CSV_Exporter** — модуль у `rustconn-core/src/export/`, що серіалізує з'єднання у CSV-формат
- **CSV_Parser** — компонент CSV_Importer, що розбирає CSV-рядки з урахуванням маппінгу колонок
- **CSV_Printer** — компонент CSV_Exporter, що форматує з'єднання у CSV-рядки
- **Mosh_Protocol** — реалізація протоколу MOSH у `rustconn-core`, аналогічна TelnetProtocol/SerialProtocol
- **Script_Resolver** — компонент у `rustconn-core`, що виконує зовнішню команду для отримання пароля
- **Session_Recorder** — компонент у `rustconn-core`, що записує VTE-вивід з мітками часу у текстовий файл
- **Recording_Format** — текстовий формат запису сесії (аналогічний script/scriptreplay)
- **Highlight_Rule** — структура у `rustconn-core`, що описує regex-правило підсвічування тексту з кольором
- **Highlight_Engine** — компонент, що застосовує Highlight_Rule до тексту терміналу
- **Broadcast_Controller** — компонент у `rustconn`, що транслює введення до вибраних відкритих терміналів
- **Smart_Folder** — структура у `rustconn-core`, що описує збережений фільтр для динамічного групування з'єднань
- **Smart_Folder_Manager** — менеджер у `rustconn-core`, що зберігає та обчислює Smart_Folder
- **SharedAppState** — `Rc<RefCell<AppState>>` — спільний стан GTK-додатку
- **SecretString** — тип із крейту `secrecy` для безпечного зберігання паролів із зануленням пам'яті при звільненні
- **CLI** — інтерфейс командного рядка `rustconn-cli`
- **i18n** — функція `i18n()` для обгортання рядків, що підлягають перекладу
- **proptest** — крейт для property-based тестування у `rustconn-core/tests/properties/`

## Вимоги

### Вимога 1: Per-connection terminal theming (P2)

**User Story:** Як системний адміністратор, я хочу налаштовувати кольори терміналу для кожного з'єднання окремо, щоб візуально розрізняти production, staging та dev сервери і запобігати помилковим діям.

#### Критерії приймання

1. THE Connection SHALL contain an optional Terminal_Theme field that stores background color, text color, and cursor color as CSS-compatible color strings
2. WHEN a user opens the Connection_Dialog, THE Connection_Dialog SHALL display Terminal_Theme fields (background color, text color, cursor color) in the Advanced tab
3. WHEN a Terminal_Theme is configured for a Connection, THE VTE_Widget SHALL apply the configured colors when creating the terminal session for that Connection
4. WHEN a Terminal_Theme is not configured for a Connection, THE VTE_Widget SHALL use the global terminal theme settings
5. WHEN a user saves a Connection with Terminal_Theme values, THE Connection SHALL persist the Terminal_Theme values across application restarts
6. THE Terminal_Theme SHALL validate that each color value is a valid CSS color string (hex format #RRGGBB or #RRGGBBAA)
7. WHEN a Terminal_Theme contains an invalid color value, THE Connection_Dialog SHALL display a validation error and prevent saving

### Вимога 2: CSV імпорт (P4)

**User Story:** Як користувач, я хочу імпортувати з'єднання з CSV-файлів, щоб легко мігрувати дані з Excel, Google Sheets або інших менеджерів з'єднань.

#### Критерії приймання

1. THE CSV_Importer SHALL parse CSV files with configurable column delimiter (comma, semicolon, tab)
2. WHEN a CSV file is provided, THE CSV_Importer SHALL detect the presence of a header row and use it for automatic column mapping
3. THE CSV_Importer SHALL support mapping CSV columns to Connection fields: name, host, port, protocol, username, group, tags, description
4. WHEN a CSV row contains an unrecognized protocol value, THE CSV_Importer SHALL skip that row and record it as a SkippedEntry with a descriptive reason
5. WHEN a CSV row is missing the required "name" or "host" field, THE CSV_Importer SHALL skip that row and record it as a SkippedEntry
6. THE CSV_Importer SHALL return an ImportResult containing the parsed connections, created groups, and import statistics
7. WHEN a CSV file contains quoted fields with embedded delimiters or newlines, THE CSV_Importer SHALL parse them correctly according to RFC 4180
8. THE CLI SHALL support CSV import via `rustconn-cli import --format csv --file <path>` with optional `--delimiter` and `--mapping` flags
9. WHEN a user imports CSV via the Connection_Dialog, THE Connection_Dialog SHALL display a column mapping preview before importing
10. FOR ALL valid CSV files, parsing then exporting then parsing SHALL produce an equivalent set of connections (round-trip property)

### Вимога 3: CSV експорт (P4)

**User Story:** Як користувач, я хочу експортувати з'єднання у CSV-формат, щоб ділитися ними з колегами або використовувати в інших інструментах.

#### Критерії приймання

1. THE CSV_Exporter SHALL serialize a list of connections into a CSV file with a header row
2. THE CSV_Exporter SHALL support field selection, allowing the user to choose which Connection fields to include in the export
3. THE CSV_Exporter SHALL export the following fields by default: name, host, port, protocol, username, group_name, tags, description
4. WHEN tags are exported, THE CSV_Exporter SHALL join multiple tags with a semicolon separator within a single CSV field
5. WHEN a field value contains the delimiter character, quotes, or newlines, THE CSV_Exporter SHALL quote the field according to RFC 4180
6. THE CLI SHALL support CSV export via `rustconn-cli export --format csv --file <path>` with optional `--fields` and `--delimiter` flags
7. THE CSV_Printer SHALL format Connection objects into valid CSV lines
8. FOR ALL valid Connection lists, exporting to CSV then importing from that CSV SHALL produce an equivalent set of connections (round-trip property)

### Вимога 4: MOSH протокол (P6)

**User Story:** Як користувач з нестабільним інтернет-з'єднанням, я хочу підключатися до серверів через MOSH, щоб зберігати сесію при зміні IP-адреси або тимчасовій втраті зв'язку.

#### Критерії приймання

1. THE ProtocolType SHALL include a Mosh variant for MOSH protocol connections
2. THE ProtocolConfig SHALL include a Mosh variant containing MoshConfig with fields: ssh_port (optional u16), mosh_port_range (optional String), server_binary (optional String), predict_mode (enum: adaptive, always, never), custom_args (Vec<String>)
3. THE Mosh_Protocol SHALL implement the Protocol trait with protocol_id "mosh", display_name "MOSH", default_port 22, and capabilities: terminal_based=true, split_view=true
4. WHEN a MOSH connection is launched, THE Mosh_Protocol SHALL build a command using the external `mosh` client binary, passing SSH port, username, host, and configured options
5. WHEN the `mosh` binary is not found on PATH, THE Mosh_Protocol SHALL return a ProtocolError::ClientNotFound error with the expected binary name
6. THE Connection_Dialog SHALL display MOSH-specific fields (SSH port, port range, predict mode, server binary) when the MOSH protocol is selected
7. WHEN a Connection with Mosh protocol is serialized and deserialized, THE Connection SHALL preserve all MoshConfig fields
8. THE Connection SHALL support default_port() returning 22 for the Mosh protocol type

### Вимога 5: Dynamic credential resolution — Script password source (P8)

**User Story:** Як DevOps-інженер, я хочу отримувати паролі через довільні скрипти або команди, щоб інтегруватися з HashiCorp Vault, AWS Secrets Manager або корпоративними API без окремого backend для кожного сервісу.

#### Критерії приймання

1. THE PasswordSource SHALL include a Script variant that stores a command string to execute for credential retrieval
2. WHEN a Connection uses PasswordSource::Script, THE Script_Resolver SHALL execute the configured command as a child process with a timeout of 30 seconds
3. WHEN the script command completes successfully, THE Script_Resolver SHALL read stdout, trim whitespace, and wrap the result in a SecretString
4. WHEN the script command exits with a non-zero exit code, THE Script_Resolver SHALL return a CredentialError with the stderr output
5. WHEN the script command exceeds the 30-second timeout, THE Script_Resolver SHALL terminate the child process and return a CredentialError indicating timeout
6. THE Script_Resolver SHALL NOT pass the command through a shell interpreter; THE Script_Resolver SHALL split the command string into program and arguments and execute directly
7. THE Connection_Dialog SHALL display a command input field when PasswordSource::Script is selected, with a placeholder showing an example command
8. WHEN a Connection with PasswordSource::Script is serialized and deserialized, THE Connection SHALL preserve the command string
9. THE Script_Resolver SHALL clear the stdout buffer from memory after wrapping it in SecretString
10. THE CLI SHALL support connecting with PasswordSource::Script, executing the configured command before establishing the connection

### Вимога 6: Session Recording (P1)

**User Story:** Як compliance-офіцер, я хочу записувати термінальні сесії з мітками часу, щоб забезпечити відповідність вимогам SOC2, ISO 27001 та PCI DSS.

#### Критерії приймання

1. THE Session_Recorder SHALL record VTE terminal output into a text-based Recording_Format file with microsecond timestamps
2. THE Recording_Format SHALL consist of two files: a data file (raw terminal output) and a timing file (timestamp + byte count per chunk), compatible with the `scriptreplay` utility format
3. WHEN session recording is enabled for a Connection, THE Session_Recorder SHALL start recording when the terminal session begins and stop when the session ends
4. THE Connection SHALL contain an optional session_recording_enabled boolean field, defaulting to false
5. THE Connection_Dialog SHALL display a "Record Session" toggle in the Advanced tab
6. WHEN session recording is active, THE VTE_Widget SHALL display a visual indicator (recording icon) in the terminal tab
7. THE Session_Recorder SHALL write recording files to a configurable directory, defaulting to `$XDG_DATA_HOME/rustconn/recordings/`
8. THE Session_Recorder SHALL name recording files using the pattern `{connection_name}_{timestamp}.{data|timing}` with sanitized connection name
9. WHEN the recording directory is not writable, THE Session_Recorder SHALL log an error and disable recording for that session without preventing the connection
10. THE Session_Recorder SHALL sanitize recorded output using the existing log sanitization patterns (passwords, API keys, tokens) before writing to disk
11. THE Recording_Format data file SHALL be parseable back into a sequence of timestamped output chunks (round-trip: write timing+data → read timing+data → identical chunks)

### Вимога 7: Text highlighting rules (P3)

**User Story:** Як системний адміністратор, я хочу підсвічувати текст у терміналі за regex-правилами, щоб швидко помічати помилки, попередження та критичні повідомлення у логах.

#### Критерії приймання

1. THE Highlight_Rule SHALL contain fields: id (Uuid), name (String), pattern (String — regex), foreground_color (optional CSS color), background_color (optional CSS color), enabled (bool)
2. THE Highlight_Engine SHALL support two scopes of rules: global rules (applied to all terminal sessions) and per-connection rules (applied only to a specific Connection)
3. WHEN a terminal line matches a Highlight_Rule pattern, THE Highlight_Engine SHALL apply the configured foreground and/or background color to the matching text region
4. WHEN multiple Highlight_Rule patterns match the same text region, THE Highlight_Engine SHALL apply the per-connection rule over the global rule, and the last matching rule within the same scope
5. THE Connection SHALL contain an optional list of Highlight_Rule for per-connection highlighting
6. THE Connection_Dialog SHALL provide a UI for managing per-connection Highlight_Rule (add, edit, delete, enable/disable)
7. WHEN a Highlight_Rule contains an invalid regex pattern, THE Highlight_Engine SHALL skip that rule and log a warning
8. THE Highlight_Rule SHALL be serializable and deserializable, preserving all fields across application restarts
9. THE Highlight_Engine SHALL provide a set of built-in default global rules for common patterns: ERROR (red), WARNING (yellow), CRITICAL (red background), FATAL (red background)

### Вимога 8: Ad-hoc broadcast (P7)

**User Story:** Як системний адміністратор, я хочу транслювати введення до довільних відкритих терміналів, щоб виконувати однакові команди на кількох серверах без попереднього створення кластера.

#### Критерії приймання

1. WHEN a user activates the Broadcast_Controller via a toolbar button, THE Broadcast_Controller SHALL display checkboxes on all open terminal tabs
2. WHEN the user selects terminal tabs and types input, THE Broadcast_Controller SHALL forward each keystroke to all selected terminals simultaneously
3. WHEN a terminal tab is closed while broadcast is active, THE Broadcast_Controller SHALL remove that terminal from the broadcast list without interrupting broadcast to remaining terminals
4. WHEN the user deactivates the Broadcast_Controller, THE Broadcast_Controller SHALL stop forwarding input and remove all checkboxes from terminal tabs
5. THE Broadcast_Controller SHALL display a visual indicator (highlighted toolbar button or banner) while broadcast mode is active
6. WHEN no terminal tabs are selected for broadcast, THE Broadcast_Controller SHALL forward input only to the currently focused terminal (normal behavior)
7. THE Broadcast_Controller SHALL support keyboard shortcut activation and deactivation

### Вимога 9: Smart Folders (P5)

**User Story:** Як користувач з великою кількістю з'єднань, я хочу створювати динамічні папки на основі фільтрів, щоб автоматично групувати з'єднання за тегами, протоколом або шаблоном хосту.

#### Критерії приймання

1. THE Smart_Folder SHALL contain fields: id (Uuid), name (String), filter_protocol (optional ProtocolType), filter_tags (Vec<String>), filter_host_pattern (optional String — glob pattern), filter_group_id (optional Uuid), sort_order (i32)
2. WHEN a Smart_Folder is evaluated, THE Smart_Folder_Manager SHALL return all connections that match ALL specified filter criteria (AND logic)
3. WHEN a Smart_Folder has no filter criteria set, THE Smart_Folder_Manager SHALL return an empty list
4. THE Smart_Folder_Manager SHALL re-evaluate Smart_Folder contents when connections are added, modified, or deleted
5. THE sidebar SHALL display Smart_Folder entries as virtual folders with a distinct icon, separate from regular groups
6. THE Connection_Dialog SHALL NOT allow moving connections into Smart_Folder (Smart_Folder are read-only virtual views)
7. WHEN a Smart_Folder filter_host_pattern is specified, THE Smart_Folder_Manager SHALL match connection hosts using glob pattern matching (supporting `*` and `?` wildcards)
8. THE Smart_Folder SHALL be serializable and deserializable, preserving all fields across application restarts
9. THE CLI SHALL support listing Smart_Folder contents via `rustconn-cli smart-folders list` and `rustconn-cli smart-folders show <name>`

### Вимога 10: Локалізація (i18n)

**User Story:** Як розробник, я хочу, щоб усі нові рядки інтерфейсу були обгорнуті функцією i18n(), щоб забезпечити переклад на 15 мов.

#### Критерії приймання

1. THE rustconn crate SHALL wrap all new user-visible strings added in v0.10.1 with the `i18n()` function
2. WHEN new translatable strings are added, THE po directory SHALL contain a `fill_i18n_0_10_1.py` script that fills translations for all 15 languages: uk, de, fr, es, it, pl, cs, sk, da, sv, nl, pt, be, kk, uz
3. THE `fill_i18n_0_10_1.py` script SHALL follow the existing pattern from `fill_i18n_0_10_0.py`, using `parse_po_file`, `extract_msgid`, `extract_msgstr`, and `rebuild_po_file` from `fill_translations.py`
4. THE `fill_i18n_0_10_1.py` script SHALL contain a TRANSLATIONS dictionary with translations for all new strings across all 15 languages

### Вимога 11: Автоматизоване тестування (proptest)

**User Story:** Як розробник, я хочу мати property-based тести для нової функціональності, щоб забезпечити коректність бізнес-логіки у `rustconn-core`.

#### Критерії приймання

1. THE rustconn-core crate SHALL contain property-based tests for CSV import/export round-trip: parsing a valid CSV then exporting and re-parsing SHALL produce equivalent connections
2. THE rustconn-core crate SHALL contain property-based tests for Terminal_Theme validation: any valid CSS hex color SHALL pass validation, and any invalid string SHALL fail
3. THE rustconn-core crate SHALL contain property-based tests for MoshConfig serialization round-trip: serializing then deserializing a MoshConfig SHALL produce an identical value
4. THE rustconn-core crate SHALL contain property-based tests for PasswordSource::Script serialization round-trip: serializing then deserializing SHALL preserve the command string
5. THE rustconn-core crate SHALL contain property-based tests for Smart_Folder filter evaluation: a connection matching all filter criteria SHALL appear in the result, and a connection not matching at least one criterion SHALL NOT appear
6. THE rustconn-core crate SHALL contain property-based tests for Highlight_Rule regex validation: a Highlight_Rule with a valid regex SHALL be accepted, and a Highlight_Rule with an invalid regex SHALL be rejected
7. THE rustconn-core crate SHALL contain property-based tests for Recording_Format round-trip: writing timing+data then reading SHALL produce identical timestamped chunks
8. THE rustconn-core crate SHALL contain property-based tests for Script_Resolver timeout: a script exceeding the timeout SHALL be terminated and return an error
9. ALL new test modules SHALL be registered in `rustconn-core/tests/properties/mod.rs`
10. ALL new tests SHALL use the `proptest!` macro with `prop_assert!` / `prop_assert_eq!` for property assertions, following the pattern in `monitoring_tests.rs`
