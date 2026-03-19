# Технічний дизайн: RustConn v0.10.1

## Огляд

Цей документ описує технічний дизайн 8 функціональних блоків для RustConn v0.10.1.
Дизайн слідує архітектурним принципам проєкту: 3-крейтовий workspace, Manager pattern,
Protocol trait, SecretString для паролів, thiserror для помилок, proptest для тестів.

**Правило розподілу:** "Чи потрібен GTK?" → Ні → `rustconn-core` / Так → `rustconn`

---

## 1. Per-connection Terminal Theming (P2)

### 1.1 Модель даних (rustconn-core)

**Файл:** `rustconn-core/src/models/connection.rs`

Нова структура `ConnectionThemeOverride` — мінімальний набір кольорів:

```rust
/// Per-connection terminal color override.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionThemeOverride {
    /// Background color (#RRGGBB or #RRGGBBAA)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// Foreground (text) color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub foreground: Option<String>,
    /// Cursor color
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}
```

Валідація у `ConnectionThemeOverride`:

```rust
impl ConnectionThemeOverride {
    pub fn validate(&self) -> Result<(), ConfigError> { /* regex #[0-9a-fA-F]{6,8} */ }
    pub fn is_empty(&self) -> bool { /* all None */ }
}
```

Нове поле у `Connection`:

```rust
pub struct Connection {
    // ... існуючі поля ...
    /// Per-connection terminal theme override
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme_override: Option<ConnectionThemeOverride>,
}
```

### 1.2 GUI (rustconn)

**Файл:** `rustconn/src/dialogs/connection/advanced_tab.rs`

- 3 `ColorDialogButton` (libadwaita) у секції "Terminal Theme" в Advanced tab
- Кнопка "Reset" для очищення override
- Preview: міні-прямокутник з обраними кольорами

**Файл:** `rustconn/src/terminal.rs` (або де створюється VTE)

При створенні VTE widget — конвертувати hex у RGBA та застосувати
`set_color_background/foreground/cursor`, або використати глобальну тему.

### 1.3 Тести (rustconn-core/tests/properties/)

**Файл:** `terminal_theme_override_tests.rs`

- `proptest!`: будь-який валідний hex `#[0-9a-fA-F]{6}` проходить валідацію
- `proptest!`: невалідний рядок (без #, неправильна довжина) не проходить
- Serde round-trip: serialize → deserialize = ідентичний результат

---

## 2. CSV Import/Export (P4)

### 2.1 CSV Import (rustconn-core)

**Файл:** `rustconn-core/src/import/csv.rs`

```rust
pub struct CsvImporter;

/// CSV column mapping configuration
#[derive(Debug, Clone)]
pub struct CsvColumnMapping {
    pub name_col: usize,
    pub host_col: usize,
    pub port_col: Option<usize>,
    pub protocol_col: Option<usize>,
    pub username_col: Option<usize>,
    pub group_col: Option<usize>,
    pub tags_col: Option<usize>,
    pub description_col: Option<usize>,
}

/// CSV parsing options
#[derive(Debug, Clone)]
pub struct CsvParseOptions {
    pub delimiter: u8,          // b',', b';', b'\t'
    pub has_header: bool,
    pub mapping: Option<CsvColumnMapping>,
}
```

Реалізує `ImportSource` trait (існуючий):
- `source_name()` → `"CSV"`
- `import_from_path_with_progress()` → парсить CSV, створює Connection

Парсинг CSV — RFC 4180 (quoted fields, embedded delimiters, newlines).
Рекомендовано крейт `csv` (serde-based) або ручний парсер.

**Автоматичний маппінг заголовків:**
Якщо перший рядок містить відомі імена колонок (`name`, `host`, `port`, `protocol`,
`username`, `group`, `tags`, `description`) — автоматичний маппінг.
Невідомі колонки ігноруються з попередженням.

**Обробка протоколів:**
Рядок `protocol` конвертується через case-insensitive matching до `ProtocolType`.
Невідомий протокол → `SkippedEntry` з причиною.

**Обробка тегів:**
Поле `tags` — розділені `;`: `"web;production;eu"` → `vec!["web","production","eu"]`

**Обробка груп:**
Поле `group` — шлях через `/`: `"Production/Web Servers"` → ієрархія `ConnectionGroup`.

**Реєстрація:** `mod csv;` + `pub use csv::CsvImporter;` у `import/mod.rs`.

### 2.2 CSV Export (rustconn-core)

**Файл:** `rustconn-core/src/export/csv.rs`

```rust
pub struct CsvExporter;

pub struct CsvExportOptions {
    pub delimiter: u8,
    pub fields: Vec<CsvExportField>,
}

pub enum CsvExportField {
    Name, Host, Port, Protocol, Username,
    GroupName, Tags, Description,
}
```

Реалізує `ExportTarget` trait. Додати `Csv` до `ExportFormat` enum.

### 2.3 CLI

- `rustconn-cli import --format csv --file data.csv --delimiter ";"`
- `rustconn-cli export --format csv --file out.csv --fields "name,host,port"`

### 2.4 Тести (`csv_tests.rs`)

- Round-trip: export → parse → еквівалентні з'єднання
- Proptest: рядки з комами/лапками/переносами коректно квотуються (RFC 4180)
- Пропущені обов'язкові поля → SkippedEntry

---

## 3. MOSH Protocol (P6)

### 3.1 Модель даних (rustconn-core/src/models/protocol.rs)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoshPredictMode { #[default] Adaptive, Always, Never }

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoshConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ssh_port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port_range: Option<String>,   // "60000:60010"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_binary: Option<String>,
    #[serde(default)]
    pub predict_mode: MoshPredictMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_args: Vec<String>,
}
```

Додати `Mosh` до `ProtocolType` та `ProtocolConfig` enums.
`default_port()` → 22 (mosh використовує SSH для handshake).

### 3.2 Protocol handler (rustconn-core/src/protocol/mosh.rs)

Аналогічно `TelnetProtocol` — мінімальна реалізація:

```rust
pub struct MoshProtocol;

impl Protocol for MoshProtocol {
    fn protocol_id(&self) -> &'static str { "mosh" }
    fn display_name(&self) -> &'static str { "MOSH" }
    fn default_port(&self) -> u16 { 22 }
    fn capabilities(&self) -> ProtocolCapabilities {
        ProtocolCapabilities::terminal() // terminal_based + split_view
    }
    fn build_command(&self, conn: &Connection) -> Option<Vec<String>> {
        // mosh [--ssh="ssh -p PORT"] [--predict=MODE]
        //      [--server=PATH] [-p PORT_RANGE] user@host
    }
}
```

Детекція: `detect_mosh()` у `detection.rs` — `which("mosh")`.

### 3.3 GUI (rustconn/src/dialogs/connection/)

Нова вкладка "MOSH" у connection dialog (аналогічно Telnet tab):
- SSH Port (SpinButton)
- Port Range (Entry, placeholder "60000:60010")
- Predict Mode (ComboRow: Adaptive/Always/Never)
- Server Binary (Entry, optional)

### 3.4 Тести (`mosh_tests.rs`)

- Serde round-trip для MoshConfig
- `build_command()` з різними комбінаціями опцій
- Валідація: порожній host → помилка

---

## 4. Dynamic Credential Resolution — Script (P8)

### 4.1 Модель (rustconn-core/src/models/connection.rs)

Новий варіант `PasswordSource::Script(String)` — зберігає команду.

### 4.2 Script Resolver (rustconn-core/src/secret/script_resolver.rs)

- Розбиває команду на program + args через `shell_words::split()`
- Виконує через `tokio::process::Command` (без shell)
- Timeout 30 секунд через `tokio::time::timeout`
- stdout → trim → `SecretString`
- Non-zero exit → `SecretError::RetrieveFailed` з stderr
- Timeout → `SecretError::RetrieveFailed` з повідомленням

### 4.3 Інтеграція

У credential resolution chain додати гілку для `PasswordSource::Script`.

### 4.4 GUI (rustconn — auth_tab.rs)

Entry для команди + placeholder + кнопка "Test".

### 4.5 Тести (`script_resolver_tests.rs`)

- Serde round-trip для `PasswordSource::Script`
- Proptest: довільний command string зберігається
- Timeout, non-zero exit, успішне виконання

---

## 5. Session Recording (P1)

### 5.1 Format (rustconn-core/src/session/recording.rs)

Сумісний з `scriptreplay`: data file (raw bytes) + timing file (delay + count).

```rust
pub struct SessionRecorder { /* data_file, timing_file, last_timestamp, sanitize */ }
pub struct RecordingReader { /* for future playback */ }
```

- `SessionRecorder::write_chunk(&[u8])` — записує з sanitization
- `RecordingReader::next_chunk()` → `Option<(Duration, Vec<u8>)>`

### 5.2 Модель

Нове поле `Connection::session_recording_enabled: bool` (default false).

### 5.3 GUI інтеграція (rustconn)

VTE `commit` callback → `recorder.write_chunk()`.
Індикатор "●REC" у заголовку вкладки.
Toggle у Advanced tab connection dialog.

### 5.4 Шлях

`$XDG_DATA_HOME/rustconn/recordings/{name}_{timestamp}.{data|timing}`

### 5.5 Тести (`recording_tests.rs`)

- Round-trip: write → read → ідентичні chunks
- Proptest: довільні байти
- Sanitization працює

---

## 6. Text Highlighting Rules (P3)

### 6.1 Модель (rustconn-core/src/models/highlight.rs)

```rust
pub struct HighlightRule {
    pub id: Uuid,
    pub name: String,
    pub pattern: String,  // regex
    pub foreground_color: Option<String>,  // #RRGGBB
    pub background_color: Option<String>,
    pub enabled: bool,
}
```

`validate_pattern()` — перевірка regex через `regex::Regex::new()`.

### 6.2 Зберігання

- Глобальні: `AppSettings::highlight_rules: Vec<HighlightRule>`
- Per-connection: `Connection::highlight_rules: Vec<HighlightRule>`
- Built-in defaults: ERROR (red), WARNING (yellow), CRITICAL/FATAL (red bg)

### 6.3 Engine (rustconn-core)

`CompiledHighlightRules::compile(global, per_conn)` — per-connection пріоритет.
`find_matches(line)` → `Vec<HighlightMatch>` з позиціями та кольорами.

### 6.4 GUI (rustconn)

VTE text attributes або overlay. Settings + Connection Dialog UI.

### 6.5 Тести (`highlight_tests.rs`)

- Proptest: валідний/невалідний regex
- Serde round-trip
- Matching positions

---

## 7. Ad-hoc Broadcast (P7)

### 7.1 Архітектура

Broadcast Controller — GUI-only компонент у `rustconn`.

### 7.2 Компонент (rustconn/src/broadcast.rs)

```rust
pub struct BroadcastController {
    active: bool,
    selected_terminals: HashSet<String>,  // session IDs
}
```

Методи: `activate()`, `deactivate()`, `toggle_terminal()`,
`is_selected()`, `remove_terminal()`, `broadcast_input()`.

### 7.3 UI Flow

1. Toolbar toggle кнопка + keyboard shortcut
2. Чекбокси на вкладках терміналів
3. Keystroke → `feed_child()` до всіх обраних
4. Деактивація → нормальний режим

### 7.4 Взаємодія з Cluster broadcast

Окремий механізм, не конфліктує з існуючим cluster broadcast.


---

## 8. Smart Folders (P5)

### 8.1 Модель даних (rustconn-core/src/models/smart_folder.rs)

```rust
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::protocol::ProtocolType;

/// A saved filter that dynamically groups connections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SmartFolder {
    pub id: Uuid,
    pub name: String,
    /// Filter by protocol type (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_protocol: Option<ProtocolType>,
    /// Filter by tags — connection must have ALL listed tags
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filter_tags: Vec<String>,
    /// Filter by host glob pattern (e.g. "*.prod.example.com")
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_host_pattern: Option<String>,
    /// Filter by parent group ID
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filter_group_id: Option<Uuid>,
    /// Display order in sidebar
    #[serde(default)]
    pub sort_order: i32,
}
```

Реєстрація: `pub mod smart_folder;` у `models/mod.rs`.

### 8.2 SmartFolderManager (rustconn-core/src/smart_folder.rs)

```rust
pub struct SmartFolderManager {
    folders: Vec<SmartFolder>,
}

impl SmartFolderManager {
    pub fn new() -> Self;
    pub fn add(&mut self, folder: SmartFolder);
    pub fn remove(&mut self, id: &Uuid) -> bool;
    pub fn get(&self, id: &Uuid) -> Option<&SmartFolder>;
    pub fn list(&self) -> &[SmartFolder];

    /// Evaluate a smart folder against a list of connections.
    /// Returns connections matching ALL filter criteria (AND logic).
    /// Empty filter criteria → empty result (not "match all").
    pub fn evaluate<'a>(
        &self,
        folder: &SmartFolder,
        connections: &'a [Connection],
    ) -> Vec<&'a Connection>;
}
```

Glob matching для `filter_host_pattern` — використати крейт `glob` або
мінімальний ручний matcher (`*` → `.*`, `?` → `.`).

Логіка `evaluate()`:
1. Якщо жоден фільтр не задано → повернути порожній вектор
2. `filter_protocol` → `conn.protocol == protocol`
3. `filter_tags` → кожен тег з фільтра присутній у `conn.tags`
4. `filter_host_pattern` → glob match `conn.host`
5. `filter_group_id` → `conn.group_id == Some(group_id)`
6. Всі активні фільтри об'єднуються через AND

### 8.3 Зберігання

Smart folders зберігаються у `AppSettings::smart_folders: Vec<SmartFolder>`.
Серіалізація/десеріалізація через serde JSON разом з іншими налаштуваннями.

### 8.4 CLI

- `rustconn-cli smart-folders list` — список усіх smart folders
- `rustconn-cli smart-folders show <name>` — з'єднання, що відповідають фільтру
- `rustconn-cli smart-folders create --name "Prod SSH" --protocol ssh --host-pattern "*.prod.*"`
- `rustconn-cli smart-folders delete <name>`

### 8.5 GUI (rustconn)

- Sidebar: окрема секція "Smart Folders" з іконкою 🔍
- Клік → список з'єднань, що відповідають фільтру (read-only, не можна drag-drop)
- Контекстне меню: Edit / Delete
- Діалог створення: ComboRow (protocol), Entry (host pattern), TagEntry (tags), GroupPicker

### 8.6 Тести (`smart_folder_tests.rs`)

- Proptest: з'єднання, що відповідає всім критеріям → присутнє у результаті
- Proptest: з'єднання, що не відповідає хоча б одному критерію → відсутнє
- Serde round-trip для SmartFolder
- Порожній фільтр → порожній результат
- Glob matching: `*.example.com` матчить `web.example.com`, не матчить `web.other.com`

---

## 9. Локалізація (i18n)

### 9.1 Скрипт `po/fill_i18n_0_10_1.py`

Слідує патерну `fill_i18n_0_10_0.py`:

```python
#!/usr/bin/env python3
"""Fill translations for new i18n strings added in RustConn 0.10.1.

Covers: MOSH protocol, CSV import/export, session recording, text highlighting,
broadcast mode, smart folders, script credentials, terminal theming.
"""

import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from fill_translations import parse_po_file, extract_msgid, extract_msgstr, rebuild_po_file

TRANSLATIONS = {
    "uk": {
        # MOSH
        "MOSH": "MOSH",
        "Predict Mode": "Режим передбачення",
        "Adaptive": "Адаптивний",
        "Always": "Завжди",
        "Never": "Ніколи",
        "SSH Port": "SSH-порт",
        "Port Range": "Діапазон портів",
        "Server Binary": "Бінарний файл сервера",
        "mosh not found": "mosh не знайдено",
        # CSV
        "CSV": "CSV",
        "Delimiter": "Роздільник",
        "Comma": "Кома",
        "Semicolon": "Крапка з комою",
        "Tab": "Табуляція",
        "Column Mapping": "Маппінг колонок",
        "Import CSV": "Імпорт CSV",
        "Export CSV": "Експорт CSV",
        # Session Recording
        "Record Session": "Запис сеансу",
        "Recording...": "Запис...",
        "Session recording enabled": "Запис сеансу увімкнено",
        "Session recording disabled": "Запис сеансу вимкнено",
        # Highlighting
        "Highlight Rules": "Правила підсвічування",
        "Add Rule": "Додати правило",
        "Pattern": "Шаблон",
        "Foreground Color": "Колір тексту",
        "Background Color": "Колір фону",
        # Broadcast
        "Broadcast Mode": "Режим трансляції",
        "Broadcast to selected terminals": "Трансляція до вибраних терміналів",
        "Select terminals for broadcast": "Виберіть термінали для трансляції",
        # Smart Folders
        "Smart Folders": "Розумні папки",
        "New Smart Folder": "Нова розумна папка",
        "Filter by Protocol": "Фільтр за протоколом",
        "Filter by Tags": "Фільтр за тегами",
        "Host Pattern": "Шаблон хосту",
        "Filter by Group": "Фільтр за групою",
        # Script credentials
        "Script": "Скрипт",
        "Command": "Команда",
        "Test Script": "Тестувати скрипт",
        "Script timeout": "Тайм-аут скрипта",
        "Script failed": "Скрипт не вдався",
        # Terminal theming
        "Terminal Theme": "Тема терміналу",
        "Background": "Фон",
        "Foreground": "Текст",
        "Cursor Color": "Колір курсора",
        "Reset Theme": "Скинути тему",
    },
    # ... (інші 14 мов за аналогією)
}
```

Повний словник для всіх 15 мов буде заповнений під час імплементації.
Структура ідентична `fill_i18n_0_10_0.py`: `fill_translations()` + `main()`.

### 9.2 Нові рядки для i18n

Усі нові рядки у крейті `rustconn` обгортаються `i18n()`:
- Мітки діалогів (MOSH tab, CSV options, Recording toggle, Highlight rules editor)
- Повідомлення про помилки (mosh not found, script timeout, invalid regex)
- Toolbar кнопки (Broadcast Mode)
- Sidebar секції (Smart Folders)
- Status bar індикатори (Recording...)

---

## 10. Наскрізні аспекти

### 10.1 Нові залежності (Cargo.toml)

| Крейт | Версія | Призначення | Де |
|--------|--------|-------------|-----|
| `csv` | 1.x | RFC 4180 парсинг/генерація | rustconn-core |
| `glob` | 0.3 | Glob matching для Smart Folders | rustconn-core |
| `shell-words` | 1.x | Розбиття команди на args | rustconn-core |

Існуючі залежності, що вже використовуються:
- `regex` — для Highlight Rules (вже є)
- `uuid` — для SmartFolder.id (вже є)
- `secrecy` — для SecretString у Script resolver (вже є)
- `tokio` — для async Script execution (вже є)
- `chrono` — для timestamps у Recording (вже є)
- `serde`/`serde_json` — для серіалізації (вже є)
- `thiserror` — для нових типів помилок (вже є)
- `proptest` — для тестів (вже є у dev-dependencies)
- `tempfile` — для тестів з файлами (вже є у dev-dependencies)

### 10.2 Нові файли

**rustconn-core:**
- `src/models/smart_folder.rs` — SmartFolder модель
- `src/models/highlight.rs` — HighlightRule модель
- `src/smart_folder.rs` — SmartFolderManager
- `src/highlight.rs` — CompiledHighlightRules engine
- `src/import/csv.rs` — CsvImporter
- `src/export/csv.rs` — CsvExporter
- `src/protocol/mosh.rs` — MoshProtocol handler
- `src/secret/script_resolver.rs` — ScriptResolver
- `src/session/recording.rs` — SessionRecorder + RecordingReader

**rustconn-core/tests/properties/:**
- `csv_tests.rs`
- `mosh_tests.rs`
- `script_resolver_tests.rs`
- `recording_tests.rs`
- `highlight_tests.rs`
- `smart_folder_tests.rs`

**rustconn (GUI):**
- `src/broadcast.rs` — BroadcastController
- Зміни у існуючих файлах діалогів

**rustconn-cli:**
- Нові subcommands: `import --format csv`, `export --format csv`, `smart-folders`

**po/:**
- `fill_i18n_0_10_1.py`

### 10.3 Зміни у існуючих файлах

| Файл | Зміна |
|------|-------|
| `rustconn-core/src/models/protocol.rs` | Додати `Mosh` до `ProtocolType`, `ProtocolConfig`; `MoshConfig`, `MoshPredictMode` |
| `rustconn-core/src/models/connection.rs` | Додати `ConnectionThemeOverride`, `theme_override`, `session_recording_enabled`, `highlight_rules`; `Script(String)` до `PasswordSource` |
| `rustconn-core/src/models/mod.rs` | `pub mod smart_folder; pub mod highlight;` |
| `rustconn-core/src/protocol/mod.rs` | `pub mod mosh;` |
| `rustconn-core/src/protocol/detection.rs` | `detect_mosh()` |
| `rustconn-core/src/import/mod.rs` | `mod csv; pub use csv::CsvImporter;` |
| `rustconn-core/src/export/mod.rs` | `pub mod csv;` + `Csv` у `ExportFormat` |
| `rustconn-core/src/secret/mod.rs` | `pub mod script_resolver;` |
| `rustconn-core/src/lib.rs` | `pub mod smart_folder; pub mod highlight;` |
| `rustconn-core/tests/properties/mod.rs` | Реєстрація 6 нових тестових модулів |
| `rustconn-core/Cargo.toml` | `csv`, `glob`, `shell-words` |

### 10.4 Міграція даних

Усі нові поля мають `#[serde(default)]` або `Option` — зворотна сумісність
забезпечена автоматично. Існуючі файли конфігурації завантажуються без змін.
Нові поля отримують значення за замовчуванням при першому завантаженні.

---

## 11. Задачі імплементації

Порядок задач враховує залежності між компонентами.

### Фаза 1 (високий пріоритет)

1. **P2: Terminal Theming — модель** — `ConnectionThemeOverride` у connection.rs, валідація, serde
2. **P2: Terminal Theming — тести** — `terminal_theme_override_tests.rs` (proptest hex validation, serde round-trip)
3. **P2: Terminal Theming — GUI** — Advanced tab color pickers, VTE integration
4. **P4: CSV Import — модель** — `CsvImporter`, `CsvColumnMapping`, `CsvParseOptions` у import/csv.rs
5. **P4: CSV Export — модель** — `CsvExporter`, `CsvExportOptions`, `CsvExportField` у export/csv.rs; `Csv` у `ExportFormat`
6. **P4: CSV — тести** — `csv_tests.rs` (round-trip, RFC 4180 quoting, missing fields)
7. **P4: CSV — CLI** — `import --format csv`, `export --format csv` subcommands
8. **P4: CSV — GUI** — Import dialog column mapping preview
9. **P6: MOSH — модель** — `MoshConfig`, `MoshPredictMode` у protocol.rs; `Mosh` у `ProtocolType`/`ProtocolConfig`
10. **P6: MOSH — protocol handler** — `MoshProtocol` у protocol/mosh.rs, `detect_mosh()`
11. **P6: MOSH — тести** — `mosh_tests.rs` (serde, build_command, validation)
12. **P6: MOSH — GUI** — MOSH tab у connection dialog
13. **P8: Script credentials — модель** — `PasswordSource::Script(String)`
14. **P8: Script credentials — resolver** — `ScriptResolver` у secret/script_resolver.rs
15. **P8: Script credentials — тести** — `script_resolver_tests.rs` (serde, timeout, exit codes)
16. **P8: Script credentials — GUI** — Auth tab command entry + Test button

### Фаза 2 (середній пріоритет)

17. **P1: Session Recording — модель** — `SessionRecorder`, `RecordingReader` у session/recording.rs
18. **P1: Session Recording — тести** — `recording_tests.rs` (round-trip, sanitization)
19. **P1: Session Recording — GUI** — Recording toggle, ●REC indicator
20. **P3: Highlighting — модель** — `HighlightRule` у models/highlight.rs, `CompiledHighlightRules` у highlight.rs
21. **P3: Highlighting — тести** — `highlight_tests.rs` (regex validation, matching, serde)
22. **P3: Highlighting — GUI** — Rules editor у Settings + Connection Dialog, VTE overlay
23. **P7: Broadcast — GUI** — `BroadcastController` у rustconn/src/broadcast.rs, toolbar toggle, tab checkboxes
24. **P5: Smart Folders — модель** — `SmartFolder` у models/smart_folder.rs, `SmartFolderManager`
25. **P5: Smart Folders — тести** — `smart_folder_tests.rs` (filter evaluation, glob, serde)
26. **P5: Smart Folders — CLI** — `smart-folders list/show/create/delete`
27. **P5: Smart Folders — GUI** — Sidebar section, create/edit dialog

### Наскрізні

28. **i18n** — `fill_i18n_0_10_1.py` з перекладами для 15 мов
29. **Реєстрація тестів** — додати всі нові модулі до `properties/mod.rs`
30. **Cargo.toml** — додати `csv`, `glob`, `shell-words` до rustconn-core dependencies
