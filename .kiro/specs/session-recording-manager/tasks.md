# Завдання: Session Recording Manager

## Блок 1: Метадані та RecordingManager (rustconn-core)

- [x] 1. Реалізувати `RecordingMetadata`, функції метаданих та `RecordingEntry` у `rustconn-core/src/session/recording.rs`
  - [x] 1.1 Додати `serde`, `serde_json` залежності у `rustconn-core/Cargo.toml` (якщо відсутні)
  - [x] 1.2 Реалізувати структуру `RecordingMetadata` з полями: `connection_name`, `display_name`, `created_at`, `duration_secs`, `total_size_bytes` та derive `Serialize`/`Deserialize`
  - [x] 1.3 Реалізувати `metadata_path(data_path) -> PathBuf` — повертає шлях `.meta.json` з data path
  - [x] 1.4 Реалізувати `read_metadata(meta_path) -> io::Result<RecordingMetadata>` — десеріалізація JSON sidecar
  - [x] 1.5 Реалізувати `write_metadata(meta_path, meta) -> io::Result<()>` — серіалізація у JSON sidecar
  - [x] 1.6 Реалізувати `derive_metadata(data_path, timing_path) -> io::Result<RecordingMetadata>` — витягнути connection_name з filename pattern `{name}_{YYYYMMDD_HHMMSS}.data`, розміри з filesystem, timestamp з modification time
  - [x] 1.7 Реалізувати структуру `RecordingEntry` з полями: `data_path`, `timing_path`, `meta_path`, `metadata`

- [x] 2. Реалізувати `RecordingManager` у `rustconn-core/src/session/recording.rs`
  - [x] 2.1 Реалізувати `RecordingManager::new(recordings_dir)` та `RecordingManager::list()` — сканувати `*.data` файли, знайти відповідні `.timing`, прочитати або derive `.meta.json`, сортувати за `created_at` desc
  - [x] 2.2 Реалізувати `RecordingManager::delete(data_path)` — видалити data + timing + meta файли
  - [x] 2.3 Реалізувати `RecordingManager::rename(data_path, new_name)` — оновити `display_name` у metadata sidecar
  - [x] 2.4 Реалізувати `RecordingManager::validate_timing(data_path, timing_path)` — перевірити формат timing рядків (`{f64} {usize}`), сума byte_count <= розмір data файлу
  - [x] 2.5 Реалізувати `RecordingManager::import(source_data, source_timing)` — validate_timing, скопіювати файли з prefix `imported_`, числовий суфікс при конфлікті, згенерувати `.meta.json`

- [x] 3. Property-based тести для метаданих та RecordingManager у `rustconn-core/tests/properties/recording_tests.rs`
  - [x] 3.1 Property 4: Metadata serde round-trip — генерувати `RecordingMetadata`, write → read → порівняти
  - [x] 3.2 Property 5: Derive metadata from filename — створити файли з pattern `{name}_{YYYYMMDD_HHMMSS}`, перевірити `derive_metadata()` повертає правильні поля
  - [x] 3.3 Property 6: Rename persists display name — створити запис, rename, read metadata, перевірити display_name
  - [x] 3.4 Property 7: Delete removes all recording files — створити 3 файли, delete, перевірити відсутність
  - [x] 3.5 Property 10: Timing file validation — генерувати валідні/невалідні timing файли, перевірити `validate_timing()`
  - [x] 3.6 Property 11: Import produces complete recording — import валідної пари, перевірити 3 файли; повторити з конфліктом імен

## Блок 2: Розширення запису (rustconn-core + rustconn)

- [x] 4. Розширити `stop_recording()` для генерації `.meta.json` sidecar у `rustconn/src/terminal/mod.rs`
  - [x] 4.1 Додати поле `recording_paths: RefCell<HashMap<Uuid, (PathBuf, PathBuf, String, Instant)>>` у `TerminalNotebook`
  - [x] 4.2 У `start_recording()` — зберегти `(data_path, timing_path, connection_name, Instant::now())` у `recording_paths`
  - [x] 4.3 У `stop_recording()` — витягнути з `recording_paths`, обчислити duration, створити `RecordingMetadata`, записати через `write_metadata()`
  - [x] 4.4 Додати `is_recording(session_id) -> bool` метод у `TerminalNotebook`
  - [x] 4.5 Додати перевірку дублікату у `start_recording()` — якщо `is_recording(session_id)` повернути `true` без дій

- [x] 5. Обробка граничних випадків запису
  - [x] 5.1 У обробнику disconnect — викликати `stop_recording(session_id)` для активних записів
  - [x] 5.2 У обробнику закриття вікна — ітерувати `session_recorders` та `flush()` для кожного
  - [x] 5.3 При помилці `write_chunk()` — логувати warning, зупинити запис, показати toast notification

## Блок 3: Sidebar Context Menu (rustconn)

- [x] 6. Додати кнопки Start/Stop Recording у контекстне меню бічної панелі
  - [x] 6.1 Розширити `show_context_menu_for_item()` у `rustconn/src/sidebar_ui.rs` — додати параметри `is_connected: bool`, `is_recording: bool`
  - [x] 6.2 Додати кнопку "Start Recording" (видима коли `is_connected && !is_recording`) з action `win.start-recording`
  - [x] 6.3 Додати кнопку "Stop Recording" (видима коли `is_connected && is_recording`) з CSS class `destructive-action` та action `win.stop-recording`
  - [x] 6.4 Зареєструвати window actions `start-recording` та `stop-recording` у відповідному обробнику
  - [x] 6.5 Оновити всі виклики `show_context_menu_for_item()` для передачі нових параметрів

## Блок 4: Recordings Dialog (rustconn)

- [x] 7. Реалізувати `RecordingsDialog` у `rustconn/src/dialogs/recording.rs`
  - [x] 7.1 Створити файл `rustconn/src/dialogs/recording.rs`, додати `pub mod recording;` у `rustconn/src/dialogs/mod.rs`
  - [x] 7.2 Реалізувати `RecordingsDialog::new(parent)` — `adw::Window` (600×450), header bar з кнопкою Import, `ListBox` з placeholder `adw::StatusPage`
  - [x] 7.3 Реалізувати `RecordingListRow` — рядок з назвою, датою, тривалістю, розміром + кнопки Play/Rename/Delete з accessible labels
  - [x] 7.4 Реалізувати `refresh_list()` — через `RecordingManager::list()` оновити ListBox
  - [x] 7.5 Реалізувати callbacks: `set_on_play`, `set_on_delete`, `set_on_rename`, `set_on_import`
  - [x] 7.6 Реалізувати Rename — inline editing або `adw::EntryRow` dialog для введення нового display_name
  - [x] 7.7 Реалізувати Delete — confirmation dialog, потім `RecordingManager::delete()`
  - [x] 7.8 Реалізувати Import — `FileChooserDialog` для вибору data + timing файлів, `RecordingManager::import()`, показати помилку при невалідних файлах

- [x] 8. Інтегрувати RecordingsDialog у App Menu та window actions
  - [x] 8.1 Додати `"Recordings..."` у Tools section `create_app_menu()` у `rustconn/src/window/ui.rs` з action `win.manage-recordings`
  - [x] 8.2 Зареєструвати window action `manage-recordings` — створити та показати `RecordingsDialog`
  - [x] 8.3 Підключити callbacks діалогу до `RecordingManager` та `PlaybackTab`

## Блок 5: Playback Tab (rustconn)

- [x] 9. Реалізувати `PlaybackController` у `rustconn/src/terminal/playback.rs`
  - [x] 9.1 Створити файл `rustconn/src/terminal/playback.rs`, додати `pub mod playback;` у `rustconn/src/terminal/mod.rs`
  - [x] 9.2 Реалізувати `PlaybackState` enum: `Idle`, `Playing`, `Stopped`, `Completed`
  - [x] 9.3 Реалізувати `PlaybackController::new()`, `load()`, `state()`
  - [x] 9.4 Реалізувати `PlaybackController::play(vte)` — ланцюжок `next_chunk()` → `glib::timeout_add_local_once(delay)` → `vte.feed(&data)` → наступний chunk
  - [x] 9.5 Реалізувати `PlaybackController::stop()` — скасувати `glib::SourceId`, стан → `Stopped`
  - [x] 9.6 Реалізувати `PlaybackController::repeat(vte)` — reset reader, clear VTE, play з початку

- [x] 10. Реалізувати Playback Tab UI у `rustconn/src/terminal/playback.rs`
  - [x] 10.1 Реалізувати `create_playback_toolbar()` — горизонтальний `GtkBox` з кнопками Clear (edit-clear-symbolic), Play (media-playback-start-symbolic), Stop (media-playback-stop-symbolic), Repeat (media-playlist-repeat-symbolic) з accessible labels
  - [x] 10.2 Реалізувати quick search filter — `SearchEntry` + `Popover` з `ListBox`, фільтрація записів по назві
  - [x] 10.3 Додати CSS клас `.playback-tab` у `rustconn/assets/style.css` для візуального виділення (overlay color, border-top)
  - [x] 10.4 Інтегрувати відкриття Playback Tab у `TerminalNotebook` — метод `open_playback_tab(recording_entry)` що створює Local Shell tab з overlay та toolbar
  - [x] 10.5 Підключити кнопки toolbar до `PlaybackController` (play/stop/repeat/clear)
  - [x] 10.6 Підключити вибір запису з quick search до `PlaybackController::load()` + автоматичний play
  - [x] 10.7 Показати індикатор завершення відтворення (зміна іконки Play, текст у toolbar)

## Блок 6: CLI recordings subcommand (rustconn-cli)

- [x] 11. Реалізувати CLI підкоманду `recordings` у `rustconn-cli`
  - [x] 11.1 Додати `RecordingCommands` enum у `rustconn-cli/src/cli.rs` з підкомандами `List`, `Delete`, `Import`
  - [x] 11.2 Додати `Recording(RecordingCommands)` варіант у `Commands` enum
  - [x] 11.3 Створити `rustconn-cli/src/commands/recording.rs` з `cmd_recording()` dispatch
  - [x] 11.4 Реалізувати `cmd_recording_list(format)` — table/json/csv вивід через `RecordingManager::list()`
  - [x] 11.5 Реалізувати `cmd_recording_delete(name, force)` — пошук за display_name або connection_name, confirmation prompt, `RecordingManager::delete()`
  - [x] 11.6 Реалізувати `cmd_recording_import(data_file, timing_file)` — `RecordingManager::import()`, вивід результату

- [x] 12. Property-based тест для CLI output у `rustconn-core/tests/properties/recording_tests.rs`
  - [x] 12.1 Property 12: CLI list output contains all metadata fields — генерувати `Vec<RecordingEntry>`, форматувати table output, перевірити наявність connection_name/date/duration/size

## Блок 7: i18n

- [x] 13. Додати i18n рядки для Session Recording Manager
  - [x] 13.1 Обгорнути всі user-visible рядки у `i18n()` у нових файлах: `recording.rs` (dialog), `playback.rs`, `sidebar_ui.rs` (нові кнопки), `window/ui.rs` (menu entry)
  - [x] 13.2 Запустити `po/update-pot.sh` для оновлення POT файлу
  - [x] 13.3 Запустити `msgmerge` для всіх `.po` файлів
  - [x] 13.4 Створити `po/fill_i18n_session_recording.py` скрипт для заповнення перекладів нових рядків у всіх мовних файлах
  - [x] 13.5 Запустити скрипт заповнення перекладів

## Блок 8: Accessibility

- [x] 14. Забезпечити доступність Session Recording Manager
  - [x] 14.1 Встановити accessible labels на всіх кнопках RecordingsDialog (Play, Rename, Delete, Import) та list items
  - [x] 14.2 Встановити accessible labels на кнопках Playback toolbar (Clear, Play, Stop, Repeat) та search entry
  - [x] 14.3 Забезпечити keyboard navigation у RecordingsDialog ListBox (вбудовано у GTK4, перевірити focus management)
  - [x] 14.4 Додати accessible status updates для зміни стану playback (Playing, Stopped, Completed)
