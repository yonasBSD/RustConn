# Документ вимог: Session Recording Manager

## Вступ

Цей документ описує вимоги до повноцінного менеджера записів сесій (Session Recording Manager) для RustConn. Функціональність охоплює три основні блоки: запис термінальних сесій, керування записами через спеціалізований діалог, та відтворення записів у виділеній вкладці Local Shell. Існуюча інфраструктура (`SessionRecorder`, `RecordingReader`, `start_recording()`/`stop_recording()` у `TerminalNotebook`) використовується як фундамент.

## Глосарій

- **Session_Recorder** — структура `SessionRecorder` у `rustconn-core/src/session/recording.rs`, що записує VTE-вивід у файли data + timing у форматі scriptreplay
- **Recording_Reader** — структура `RecordingReader` у `rustconn-core/src/session/recording.rs`, що зчитує записані сесії порціями з таймінгом
- **Recording** — пара файлів (data + timing) у `$XDG_DATA_HOME/rustconn/recordings/`, що представляє один запис сесії
- **Recording_Metadata** — структура метаданих запису: назва з'єднання, дата створення, тривалість, розмір файлів, користувацька назва
- **Recordings_Dialog** — GTK4/libadwaita діалог керування записами (аналогічний ClusterDialog, TemplateDialog)
- **Playback_Tab** — спеціалізована вкладка Local Shell для відтворення записів із візуальним виділенням та панеллю керування
- **Playback_Controller** — компонент, що керує відтворенням запису (play, stop, repeat) через Recording_Reader
- **Sidebar_Context_Menu** — контекстне меню правої кнопки миші на з'єднаннях у бічній панелі (`rustconn/src/sidebar_ui.rs`)
- **App_Menu** — головне меню-гамбургер додатку (`rustconn/src/window/ui.rs`, функція `create_app_menu()`)
- **Terminal_Notebook** — компонент `TerminalNotebook` у `rustconn/src/terminal/mod.rs`, що керує вкладками терміналів
- **SanitizeConfig** — конфігурація санітизації виводу у `rustconn-core/src/session/logger.rs`
- **VTE_Widget** — віджет терміналу VTE4 у крейті `rustconn`
- **i18n** — функція `i18n()` для обгортання рядків, що підлягають перекладу
- **CLI** — інтерфейс командного рядка `rustconn-cli`

## Вимоги

### Вимога 1: Запуск запису з контекстного меню бічної панелі

**User Story:** Як системний адміністратор, я хочу запускати запис вже підключеної сесії через контекстне меню бічної панелі, щоб зафіксувати важливі дії без необхідності перепідключення.

#### Критерії приймання

1. WHEN a user right-clicks on a connected session in the sidebar, THE Sidebar_Context_Menu SHALL display a "Start Recording" button
2. WHEN a user clicks "Start Recording" for a connected session, THE Terminal_Notebook SHALL invoke `start_recording()` for the corresponding session_id
3. WHILE a session is being recorded, THE Sidebar_Context_Menu SHALL display "Stop Recording" instead of "Start Recording" for that session
4. WHEN a user clicks "Stop Recording", THE Terminal_Notebook SHALL invoke `stop_recording()` for the corresponding session_id and flush all buffered data to disk
5. IF `start_recording()` fails due to an unwritable recordings directory, THEN THE Terminal_Notebook SHALL display an error notification to the user
6. WHEN a session is not connected, THE Sidebar_Context_Menu SHALL hide the "Start Recording" / "Stop Recording" button for that item

### Вимога 2: Автоматичний запис при підключенні

**User Story:** Як користувач, я хочу налаштувати автоматичний запис для конкретного з'єднання, щоб кожна сесія записувалась без ручного втручання.

#### Критерії приймання

1. THE Connection_Dialog SHALL contain a "Record Session" toggle in the Advanced tab that enables automatic recording on connect
2. WHEN a connection with "Record Session" enabled is established, THE Terminal_Notebook SHALL automatically invoke `start_recording()` for the new session
3. WHEN a recorded session disconnects, THE Terminal_Notebook SHALL automatically invoke `stop_recording()` to flush and finalize the recording
4. IF the recordings directory is unavailable when auto-recording starts, THEN THE Terminal_Notebook SHALL log a warning and proceed with the connection without recording

### Вимога 3: Збереження записів у форматі scriptreplay

**User Story:** Як DevOps-інженер, я хочу щоб записи зберігались у стандартному форматі scriptreplay, щоб я міг відтворювати їх зовнішніми інструментами.

#### Критерії приймання

1. THE Session_Recorder SHALL write terminal output to a data file and timing information to a separate timing file in scriptreplay-compatible format
2. THE Session_Recorder SHALL store recordings in the `$XDG_DATA_HOME/rustconn/recordings/` directory
3. THE Session_Recorder SHALL name recording files using the pattern `{sanitized_connection_name}_{UTC_timestamp}.data` and `{sanitized_connection_name}_{UTC_timestamp}.timing`
4. WHEN sanitization is enabled, THE Session_Recorder SHALL redact sensitive data (passwords, credentials) from the recording output before writing to disk
5. FOR ALL valid recordings, reading then concatenating all chunks SHALL produce byte-identical output to the original data file (round-trip property)
6. THE Session_Recorder SHALL skip empty chunks without writing timing entries

### Вимога 4: Діалог керування записами

**User Story:** Як користувач, я хочу мати зручний діалог для перегляду, перейменування, видалення та імпорту записів, щоб організовувати свою бібліотеку записів.

#### Критерії приймання

1. THE App_Menu SHALL contain a "Recordings..." entry in the Tools section alongside Clusters, Templates, Snippets, and Variables
2. WHEN a user opens the Recordings_Dialog, THE Recordings_Dialog SHALL display a scrollable list of all recordings with Recording_Metadata (connection name, user-defined name, date, duration, file size)
3. WHEN a user selects a recording and clicks "Rename", THE Recordings_Dialog SHALL allow the user to set a custom display name stored in the metadata sidecar file
4. WHEN a user selects a recording and clicks "Delete", THE Recordings_Dialog SHALL prompt for confirmation and then remove the data file, timing file, and metadata sidecar file from disk
5. WHEN a user clicks "Import", THE Recordings_Dialog SHALL open a file chooser for selecting external scriptreplay-compatible files (data + timing pair) and copy them into the recordings directory
6. IF an imported file pair is not valid scriptreplay format, THEN THE Recordings_Dialog SHALL display an error message describing the validation failure
7. WHEN a user selects a recording and clicks "Play", THE Recordings_Dialog SHALL open a Playback_Tab and begin playback of the selected recording
8. WHEN the recordings directory is empty, THE Recordings_Dialog SHALL display a placeholder message indicating no recordings are available

### Вимога 5: Метадані записів

**User Story:** Як користувач, я хочу бачити детальну інформацію про кожен запис, щоб швидко знаходити потрібний.

#### Критерії приймання

1. THE Recording_Metadata SHALL include: original connection name, user-defined display name, creation timestamp, recording duration, combined file size of data and timing files
2. WHEN a recording is created, THE Session_Recorder SHALL generate a metadata sidecar file in JSON format (`.meta.json`) alongside the data and timing files
3. THE Recording_Metadata parser SHALL read JSON sidecar files and produce Recording_Metadata structures
4. THE Recording_Metadata printer SHALL format Recording_Metadata structures back into valid JSON sidecar files
5. FOR ALL valid Recording_Metadata structures, parsing then printing then parsing SHALL produce an equivalent structure (round-trip property)
6. WHEN a recording exists without a metadata sidecar file (legacy or externally imported), THE Recordings_Dialog SHALL derive metadata from the filename pattern and file system attributes (creation date, file size)

### Вимога 6: Відтворення записів у вкладці Playback

**User Story:** Як користувач, я хочу відтворювати записи у спеціальній вкладці з візуальним виділенням, щоб чітко відрізняти відтворення від живих сесій.

#### Критерії приймання

1. WHEN playback is initiated, THE Terminal_Notebook SHALL open a new Local Shell tab designated as a Playback_Tab
2. THE Playback_Tab SHALL apply a distinct visual overlay or background color (CSS class) to differentiate the tab from regular connected sessions
3. THE Playback_Tab SHALL display a control panel toolbar with the following buttons: Clear, Play, Stop, Repeat, and a recording selector with quick search filter
4. WHEN a user clicks "Play", THE Playback_Controller SHALL begin feeding recorded chunks to the VTE_Widget respecting the original timing delays between chunks
5. WHEN a user clicks "Stop", THE Playback_Controller SHALL pause playback at the current position
6. WHEN a user clicks "Repeat", THE Playback_Controller SHALL reset playback to the beginning and start playing from the first chunk
7. WHEN a user clicks "Clear", THE Playback_Tab SHALL clear the VTE_Widget terminal content
8. WHEN a user selects a different recording from the quick search filter, THE Playback_Controller SHALL stop current playback, clear the terminal, and load the selected recording
9. WHEN playback reaches the end of the recording, THE Playback_Controller SHALL stop and display a visual indicator that playback is complete

### Вимога 7: Індикація стану запису

**User Story:** Як користувач, я хочу чітко бачити які сесії зараз записуються, щоб не забути зупинити запис.

#### Критерії приймання

1. WHILE a session is being recorded, THE Terminal_Notebook SHALL display a "●REC" prefix in the tab title of the recorded session
2. WHEN recording starts, THE Terminal_Notebook SHALL add the "●REC" indicator to the tab title
3. WHEN recording stops, THE Terminal_Notebook SHALL remove the "●REC" indicator from the tab title
4. WHILE a session is being recorded, THE Sidebar_Context_Menu SHALL visually distinguish the "Stop Recording" button using a destructive CSS class

### Вимога 8: Обробка граничних випадків запису

**User Story:** Як користувач, я хочу щоб система коректно обробляла нестандартні ситуації під час запису, щоб не втрачати дані.

#### Критерії приймання

1. WHEN a connection disconnects while recording is active, THE Terminal_Notebook SHALL automatically invoke `stop_recording()` to flush and finalize the recording
2. IF the disk runs out of space during recording, THEN THE Session_Recorder SHALL stop recording, flush buffered data, and notify the user via an error message
3. WHEN a user attempts to start recording on a session that is already being recorded, THE Terminal_Notebook SHALL ignore the duplicate request and keep the existing recording active
4. IF the recordings directory is deleted while a recording is in progress, THEN THE Session_Recorder SHALL handle the write error gracefully and notify the user
5. WHEN the application is closed while recordings are active, THE Terminal_Notebook SHALL invoke `stop_recording()` for all active recordings before shutdown

### Вимога 9: Імпорт зовнішніх записів

**User Story:** Як користувач, я хочу імпортувати записи, створені зовнішніми інструментами (script/scriptreplay), щоб переглядати їх у RustConn.

#### Критерії приймання

1. WHEN a user initiates import, THE Recordings_Dialog SHALL accept a pair of files: one data file and one timing file
2. THE Recordings_Dialog SHALL validate that the timing file contains well-formed timing entries (floating-point delay and integer byte count per line)
3. THE Recordings_Dialog SHALL validate that the sum of byte counts in the timing file does not exceed the data file size
4. WHEN validation succeeds, THE Recordings_Dialog SHALL copy both files into the recordings directory and generate a Recording_Metadata sidecar file
5. IF the imported files have names that conflict with existing recordings, THEN THE Recordings_Dialog SHALL append a numeric suffix to avoid overwriting

### Вимога 10: Інтернаціоналізація

**User Story:** Як користувач, я хочу щоб усі нові елементи інтерфейсу були перекладені, щоб використовувати додаток рідною мовою.

#### Критерії приймання

1. THE Session Recording Manager SHALL wrap all user-visible strings with the `i18n()` function
2. THE Session Recording Manager SHALL include translatable strings for: menu entries, button labels, dialog titles, error messages, placeholder text, tooltip text, and accessibility labels
3. WHEN new translatable strings are added, THE build system SHALL include them in the POT file for translator extraction

### Вимога 11: Підтримка CLI

**User Story:** Як досвідчений користувач, я хочу керувати записами через командний рядок, щоб автоматизувати роботу з ними.

#### Критерії приймання

1. THE CLI SHALL provide a `recordings list` subcommand that displays all recordings with metadata in tabular format
2. THE CLI SHALL provide a `recordings delete <name>` subcommand that removes a recording by name with confirmation prompt
3. THE CLI SHALL provide a `recordings import <data_file> <timing_file>` subcommand that imports an external scriptreplay recording pair
4. IF a CLI recording command references a non-existent recording, THEN THE CLI SHALL display a descriptive error message and exit with a non-zero status code

### Вимога 12: Доступність (Accessibility)

**User Story:** Як користувач з обмеженими можливостями, я хочу щоб менеджер записів був доступний через допоміжні технології.

#### Критерії приймання

1. THE Recordings_Dialog SHALL set accessible labels on all interactive widgets (buttons, list items, dropdowns)
2. THE Playback_Tab control panel SHALL set accessible labels on all playback control buttons describing their function
3. THE Playback_Tab SHALL announce playback state changes (playing, stopped, completed) via accessible status updates
4. THE Recordings_Dialog list SHALL support keyboard navigation for selecting, renaming, and deleting recordings
