# 📦 Задачі на релізи RustConn

**Базова версія:** 0.10.20 | **Створено:** Квітень 2026  
**Джерело:** Консолідований аудит (UX/HIG, Security, Architecture)

---

## v0.10.21 — Security Hardening

### TASK-001: Видалити слабкий fallback машинного ключа шифрування

**Пріоритет:** 🔴 Критичний | **Оцінка:** 2-4 год | **Область:** Security  
**Файл:** `rustconn-core/src/config/settings.rs` (функція `get_machine_key()`)

**Проблема:**  
Третій fallback у `get_machine_key()` генерує ключ як `{hostname}-{username}-rustconn-key` — повністю передбачуваний. Зловмисник з доступом до файлу конфігурації може відтворити ключ шифрування.

**Поточний код:**
```rust
// 3. Fallback to hostname + username
let hostname = hostname::get().map_or_else(
    |_| "rustconn".to_string(),
    |h| h.to_string_lossy().to_string(),
);
let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
format!("{hostname}-{username}-rustconn-key").into_bytes()
```

**Що зробити:**

1. Видалити третій fallback повністю — замінити на `return` з помилкою:
```rust
// 3. No fallback — refuse to encrypt with predictable key
tracing::error!(
    "Cannot derive encryption key: no .machine-key file and /etc/machine-id unavailable. \
     Credential encryption disabled."
);
Vec::new() // Empty key signals "encryption unavailable"
```

2. Для другого fallback (`/etc/machine-id`) додати HKDF з app-specific salt:
```rust
// 2. Try /etc/machine-id with HKDF derivation
if let Ok(machine_id) = std::fs::read_to_string("/etc/machine-id") {
    let trimmed = machine_id.trim().as_bytes();
    // Derive app-specific key via HKDF to avoid sharing raw machine-id
    let salt = b"rustconn-machine-key-v1";
    let mut derived = vec![0u8; 32];
    ring::hkdf::Salt::new(ring::hkdf::HKDF_SHA256, salt)
        .extract(trimmed)
        .expand(&[b"encryption"], &ring::hkdf::HKDF_SHA256)
        .map(|okm| okm.fill(&mut derived));
    return derived;
}
```

3. Встановити права `0600` на `.machine-key` при створенні:
```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(&key_file, std::fs::Permissions::from_mode(0o600));
}
```

4. Додати обробку випадку "порожній ключ" у `encrypt_credential()` / `decrypt_credential()` — повертати помилку замість шифрування з порожнім ключем.

**Тестування:**
- Property test: `get_machine_key()` ніколи не повертає порожній вектор при наявності `.machine-key` або `/etc/machine-id`
- Unit test: перевірити що HKDF derivation дає стабільний результат для одного machine-id
- Unit test: перевірити що `.machine-key` створюється з правами 0600

---

### TASK-002: Встановити права 0600 на файл конфігурації

**Пріоритет:** 🔴 Критичний | **Оцінка:** 1-2 год | **Область:** Security  
**Файл:** `rustconn-core/src/config/settings.rs`

**Проблема:**  
Файл `settings.toml` містить поля `*_encrypted` (bitwarden_password_encrypted, kdbx_password_encrypted тощо), зашифровані AES-256-GCM. Але файл може мати стандартні права 0644, доступні іншим користувачам.

**Що зробити:**

1. Після кожного збереження `settings.toml` встановлювати обмежені права:
```rust
fn save_settings_file(path: &Path, content: &str) -> Result<(), ConfigError> {
    // Atomic write with fsync (existing logic)
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, content)?;
    let file = std::fs::File::open(&tmp)?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)?;

    // Restrict permissions to owner-only
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}
```

2. При завантаженні — перевіряти та логувати попередження:
```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    if let Ok(metadata) = std::fs::metadata(&settings_path) {
        let mode = metadata.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            tracing::warn!(
                path = %settings_path.display(),
                mode = format!("{mode:o}"),
                "Settings file has overly permissive permissions, fixing to 0600"
            );
            let _ = std::fs::set_permissions(
                &settings_path,
                std::fs::Permissions::from_mode(0o600),
            );
        }
    }
}
```

**Тестування:**
- Unit test з `tempfile`: створити settings, зберегти, перевірити permissions
- Unit test: перевірити що попередження логується при mode 0644

---

### TASK-003: Локалізувати accessible labels у sidebar

**Пріоритет:** 🔴 Критичний | **Оцінка:** 30 хв | **Область:** Accessibility / i18n  
**Файли:** `rustconn/src/sidebar/mod.rs`, `rustconn/src/sidebar/filter.rs`

**Проблема:**  
Screen reader оголошує англійський текст незалежно від мови інтерфейсу. Три accessible labels не обгорнуті в `i18n()`.

**Поточний код (sidebar/mod.rs:124):**
```rust
search_entry.update_property(&[gtk4::accessible::Property::Label("Search connections")]);
```

**Виправлення (sidebar/mod.rs:124):**
```rust
search_entry.update_property(&[gtk4::accessible::Property::Label(&i18n("Search connections"))]);
```

**Поточний код (sidebar/mod.rs:148):**
```rust
help_button.update_property(&[gtk4::accessible::Property::Label("Search syntax help")]);
```

**Виправлення (sidebar/mod.rs:148):**
```rust
help_button.update_property(&[gtk4::accessible::Property::Label(&i18n("Search syntax help"))]);
```

**Поточний код (sidebar/mod.rs:565):**
```rust
list_view.update_property(&[gtk4::accessible::Property::Label("Connection list")]);
```

**Виправлення (sidebar/mod.rs:565):**
```rust
list_view.update_property(&[gtk4::accessible::Property::Label(&i18n("Connection list"))]);
```

**Поточний код (sidebar/filter.rs:25):**
```rust
let accessible_label = format!("Filter by {protocol} protocol");
```

**Виправлення (sidebar/filter.rs:25):**
```rust
let accessible_label = i18n_f("Filter by {} protocol", &[protocol]);
```

**Після виправлення:** Оновити `po/POTFILES.in` якщо файли ще не включені, та запустити `po/update-pot.sh` для оновлення `.pot` файлу.

---

## v0.11.0 — UX Modernization & Security Improvements

### TASK-004: Міграція General tab діалогу підключення на adw:: widgets

**Пріоритет:** 🟠 Високий | **Оцінка:** 8-16 год | **Область:** UX / GNOME HIG  
**Файли:** `rustconn/src/dialogs/connection/general_tab.rs`, `rustconn/src/dialogs/connection/dialog.rs`

**Проблема:**  
General tab — перше, що бачить користувач при створенні підключення. Він побудований на `gtk4::Grid` з ручними `Label` + `Entry` парами, тоді як Advanced tab та SSH tab вже використовують нативні `adw::EntryRow`, `adw::ComboRow`, `adw::SpinRow`. Це створює візуальну невідповідність між вкладками одного діалогу.

Додатково, `create_basic_tab()` повертає tuple з 30 елементів — це ускладнює підтримку.

**Поточний код (general_tab.rs:15-50):**
```rust
#[allow(clippy::type_complexity)]
pub(super) fn create_basic_tab() -> (
    GtkBox, Entry, Entry, TextView, Entry, Label, SpinButton, Label,
    // ... ще 22 елементи
) {
    let grid = Grid::builder().row_spacing(8).column_spacing(12).build();
    let name_label = Label::builder().label(i18n("Name:")).halign(gtk4::Align::End).build();
    let name_entry = Entry::builder().placeholder_text(i18n("Connection name")).build();
    grid.attach(&name_label, 0, row, 1, 1);
    grid.attach(&name_entry, 1, row, 2, 1);
}
```

**Що зробити:**

1. Створити struct замість tuple:
```rust
pub(super) struct BasicTabWidgets {
    pub container: GtkBox,
    pub name_entry: adw::EntryRow,
    pub icon_entry: adw::EntryRow,
    pub description: gtk4::TextView,
    pub host_entry: adw::EntryRow,
    pub port_spin: adw::SpinRow,
    pub username_entry: adw::EntryRow,
    pub domain_entry: adw::EntryRow,
    pub password_source: adw::ComboRow,
    pub password_entry: adw::PasswordEntryRow,
    pub protocol_dropdown: adw::ComboRow,
    pub group_dropdown: adw::ComboRow,
    pub tags_entry: adw::EntryRow,
    pub variable_entry: adw::EntryRow,
    // ... інші поля
}
```

2. Замінити Grid на `adw::PreferencesGroup` з секціями Identity / Connection / Authentication / Organization:
```rust
pub(super) fn create_basic_tab() -> BasicTabWidgets {
    let vbox = GtkBox::new(Orientation::Vertical, 0);

    let identity_group = adw::PreferencesGroup::builder()
        .title(i18n("Identity")).build();
    let name_entry = adw::EntryRow::builder().title(i18n("Name")).build();
    identity_group.add(&name_entry);
    vbox.append(&identity_group);

    let connection_group = adw::PreferencesGroup::builder()
        .title(i18n("Connection")).build();
    let host_entry = adw::EntryRow::builder().title(i18n("Host")).build();
    let port_spin = adw::SpinRow::builder()
        .title(i18n("Port"))
        .adjustment(&gtk4::Adjustment::new(22.0, 1.0, 65535.0, 1.0, 10.0, 0.0))
        .build();
    connection_group.add(&host_entry);
    connection_group.add(&port_spin);
    vbox.append(&connection_group);

    let auth_group = adw::PreferencesGroup::builder()
        .title(i18n("Authentication")).build();
    let username_entry = adw::EntryRow::builder().title(i18n("Username")).build();
    let password_entry = adw::PasswordEntryRow::builder().title(i18n("Password")).build();
    auth_group.add(&username_entry);
    auth_group.add(&password_entry);
    vbox.append(&auth_group);

    BasicTabWidgets { container: vbox, name_entry, icon_entry, /* ... */ }
}
```

3. Оновити всі місця в `dialog.rs`, що деструктурують tuple, на struct полів.

**Ризик:** Високий — перевірити всі 6 джерел паролів, inline validation, tab order, accessible relations, всі протоколи.

**Тестування:** Ручне тестування кожного протоколу + screen reader (Orca).

---

### TASK-005: Посилити валідацію automation tasks

**Пріоритет:** 🟠 Високий | **Оцінка:** 4-6 год | **Область:** Security  
**Файли:** `rustconn-core/src/automation/tasks.rs`, `rustconn/src/automation.rs`, `rustconn-core/tests/properties/`

**Проблема:**  
`TaskExecutor::execute_command()` виконує команди через `sh -c` (tasks.rs:468-471). Expect-правила дозволяють відправляти довільний текст у термінал.

**Що зробити:**

1. Додати property-based тести для `validate_command_value()`:
```rust
// rustconn-core/tests/properties/automation_security_tests.rs
proptest! {
    #[test]
    fn validate_rejects_shell_metacharacters(value in ".*[;|&`$()<>!].*") {
        assert!(validate_command_value(&value).is_err());
    }

    #[test]
    fn validate_accepts_safe_strings(value in "[a-zA-Z0-9_./ -]{1,100}") {
        assert!(validate_command_value(&value).is_ok());
    }
}
```

2. Попередження при імпорті з'єднань з automation/expect правилами:
```rust
if !connection.automation_tasks.is_empty() || !connection.expect_rules.is_empty() {
    warnings.push(ImportWarning::AutomationPresent {
        connection_name: connection.name.clone(),
        task_count: connection.automation_tasks.len(),
        expect_count: connection.expect_rules.len(),
    });
}
```

3. Очищувати чутливі env vars перед запуском задач:
```rust
let mut cmd = Command::new("sh");
cmd.arg("-c").arg(command);
for var in &["BW_SESSION", "AWS_SECRET_ACCESS_KEY", "AWS_SESSION_TOKEN"] {
    cmd.env_remove(var);
}
```

4. Зареєструвати тестовий модуль в `rustconn-core/tests/properties/mod.rs`.

---

### TASK-006: Попередження та план видалення XOR-шифру

**Пріоритет:** 🟠 Високий | **Оцінка:** 2-3 год | **Область:** Security  
**Файл:** `rustconn-core/src/config/settings.rs`

**Проблема:**  
`decrypt_credential()` підтримує застарілий XOR-формат (тривіально зворотній).

**v0.11 — додати лічильник міграцій:**
```rust
// В decrypt_credential(), гілка legacy:
tracing::warn!("Legacy XOR encryption detected — migrating to AES-256-GCM. \
    XOR support will be removed in v0.12.");
LEGACY_MIGRATION_COUNT.fetch_add(1, Ordering::Relaxed);
```

**v0.11 — показати Toast після завантаження:**
```rust
if legacy_migration_count() > 0 {
    show_toast(&overlay,
        &i18n_f("{} credentials migrated from legacy encryption to AES-256-GCM",
            &[&legacy_migration_count().to_string()]),
        ToastLevel::Info);
}
```

**v0.12 — видалити `xor_cipher_legacy()` та fallback гілку повністю.** Задокументувати в CHANGELOG.

---

### TASK-007: Структуризувати RDP connection state

**Пріоритет:** 🟠 Високий | **Оцінка:** 2-3 год | **Область:** Architecture  
**Файл:** `rustconn/src/embedded_rdp/connection.rs`

**Проблема:**  
`handle_ironrdp_error()` приймає 13 параметрів `Rc<RefCell<...>>`.

**Що зробити — створити context struct:**
```rust
pub(crate) struct RdpConnectionContext {
    pub state: Rc<RefCell<RdpConnectionState>>,
    pub drawing_area: gtk4::DrawingArea,
    pub toolbar: gtk4::Box,
    pub on_state_changed: Rc<RefCell<Option<super::types::StateCallback>>>,
    pub on_error: Rc<RefCell<Option<super::types::ErrorCallback>>>,
    pub on_fallback: Rc<RefCell<Option<super::types::FallbackCallback>>>,
    pub is_embedded: Rc<RefCell<bool>>,
    pub is_ironrdp: Rc<RefCell<bool>>,
    pub ironrdp_tx: Rc<RefCell<Option<std::sync::mpsc::Sender<RdpClientCommand>>>>,
    pub client_ref: Rc<RefCell<Option<rustconn_core::rdp_client::RdpClient>>>,
    pub fallback_config: Rc<RefCell<Option<RdpConfig>>>,
    pub fallback_process: Rc<RefCell<Option<std::process::Child>>>,
    pub clipboard_handler_id: Rc<RefCell<Option<glib::SignalHandlerId>>>,
}

fn handle_ironrdp_error(msg: &str, ctx: &RdpConnectionContext) { /* ... */ }
```

**Тестування:** `cargo check --all-targets` — зміна чисто структурна.

---

### TASK-008: Синхронізувати документацію з кодом (with_state хелпери)

**Пріоритет:** 🟠 Високий | **Оцінка:** 2-4 год | **Область:** Architecture / DX  
**Файли:** `docs/ARCHITECTURE.md`, `rustconn/src/state.rs`

**Проблема:**  
`ARCHITECTURE.md` документує `with_state()` / `try_with_state()` / `with_state_mut()` / `try_with_state_mut()`, але вони не існують у коді.

**Рекомендований варіант — реалізувати хелпери:**
```rust
// rustconn/src/state.rs
pub fn with_state<R>(state: &SharedAppState, f: impl FnOnce(&AppState) -> R) -> R {
    f(&state.borrow())
}

pub fn try_with_state<R>(state: &SharedAppState, f: impl FnOnce(&AppState) -> R) -> Option<R> {
    state.try_borrow().ok().map(|s| f(&*s))
}

pub fn with_state_mut<R>(state: &SharedAppState, f: impl FnOnce(&mut AppState) -> R) -> R {
    f(&mut state.borrow_mut())
}

pub fn try_with_state_mut<R>(
    state: &SharedAppState, f: impl FnOnce(&mut AppState) -> R,
) -> Option<R> {
    state.try_borrow_mut().ok().map(|mut s| f(&mut *s))
}
```

**Альтернатива:** Видалити згадку хелперів з `ARCHITECTURE.md`.

---

## v0.12.0 — Adaptive UI & Quality

### TASK-009: Адаптивність діалогу підключення

**Пріоритет:** 🟡 Середній | **Оцінка:** 4-6 год | **Область:** UX / Responsive  
**Залежність:** TASK-004 (міграція на adw:: widgets)  
**Файл:** `rustconn/src/dialogs/connection/dialog.rs`

**Проблема:**  
Діалог підключення використовує фіксовану ширину. На вузьких екранах (планшет, Phosh) контент може обрізатися.

**Що зробити:**  
Після міграції General tab на `adw::PreferencesGroup` (TASK-004), додати `AdwBreakpoint` для адаптивного layout:
```rust
// Якщо діалог використовує adw::Dialog (libadwaita >= 1.5):
let breakpoint = adw::Breakpoint::new(
    adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        500.0,
        adw::LengthUnit::Sp,
    ),
);
// При вузькому вікні — зменшити padding, сховати необов'язкові поля
breakpoint.add_setter(&description_row, "visible", &false.to_value());
dialog.add_breakpoint(breakpoint);
```

Якщо діалог використовує `adw::Window` — обгорнути контент у `adw::Clamp` з `maximum_size(600)`.

**Тестування:** Перевірити при ширині вікна 360px, 500px, 768px, 1024px.

---

### TASK-010: VNC security warnings

**Пріоритет:** 🟡 Середній | **Оцінка:** 4-8 год | **Область:** Security / UX  
**Файли:** `rustconn/src/dialogs/connection/`, `rustconn-core/src/protocol/vnc.rs`

**Проблема:**  
VNC протокол не забезпечує шифрування трафіку. Паролі передаються через слабку DES-автентифікацію RFB. Це обмеження протоколу, але користувач має бути поінформований.

**Що зробити:**

1. Додати інформаційний банер у діалозі підключення при виборі VNC:
```rust
// При зміні протоколу на VNC:
let vnc_warning = adw::Banner::builder()
    .title(i18n("VNC traffic is unencrypted. Consider using SSH tunnel for security."))
    .button_label(i18n("Learn more"))
    .revealed(true)
    .build();
vnc_warning.add_css_class("warning");
```

2. Додати опцію "Use SSH tunnel" в VNC protocol tab:
```rust
let ssh_tunnel_switch = adw::SwitchRow::builder()
    .title(i18n("SSH Tunnel"))
    .subtitle(i18n("Route VNC through encrypted SSH connection"))
    .build();
```

3. Документувати ризики в User Guide (секція VNC).

---

### TASK-011: Accessible relations у widget builders

**Пріоритет:** 🟡 Середній | **Оцінка:** 2-3 год | **Область:** Accessibility  
**Файл:** `rustconn/src/dialogs/widgets.rs`

**Проблема:**  
`EntryRowBuilder`, `SpinRowBuilder`, `DropdownRowBuilder` не встановлюють `Relation::LabelledBy` між suffix-віджетами та ActionRow title.

**Що зробити:**  
У кожному builder, після створення suffix widget, додати accessible relation:
```rust
// В EntryRowBuilder::build():
let entry = Entry::builder()./* ... */.build();
// Link entry to the row's title for screen readers
entry.update_relation(&[
    gtk4::accessible::Relation::LabelledBy(&[row.upcast_ref::<gtk4::Accessible>()])
]);
row.add_suffix(&entry);
```

Аналогічно для `SpinRowBuilder` та `DropdownRowBuilder`.

---

### TASK-012: Runtime warning для block_on_async

**Пріоритет:** 🟡 Середній | **Оцінка:** 1 год | **Область:** Architecture / DX  
**Файл:** `rustconn/src/async_utils.rs`

**Проблема:**  
`block_on_async` блокує GTK main thread. Немає runtime guard для виявлення тривалих блокувань.

**Що зробити:**
```rust
pub fn block_on_async<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    let start = std::time::Instant::now();
    let result = TOKIO_RUNTIME.with(|rt| {
        let mut rt = rt.borrow_mut();
        let runtime = rt.get_or_insert_with(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime")
        });
        runtime.block_on(future)
    });
    let elapsed = start.elapsed();
    if elapsed > std::time::Duration::from_millis(100) {
        tracing::warn!(
            elapsed_ms = elapsed.as_millis(),
            "block_on_async blocked GTK main thread for >100ms — \
             consider using spawn_async instead"
        );
    }
    result
}
```

---

### TASK-013: Інтегрувати cargo audit у CI

**Пріоритет:** 🟡 Середній | **Оцінка:** 2-3 год | **Область:** Security / CI  
**Файли:** `.github/workflows/`

**Що зробити:**

1. Додати job у GitHub Actions:
```yaml
security-audit:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: rustsec/audit-check@v2
      with:
        token: ${{ secrets.GITHUB_TOKEN }}
```

2. Додати `cargo-deny` для політики залежностей:
```yaml
    - name: Install cargo-deny
      run: cargo install cargo-deny
    - name: Check dependencies
      run: cargo deny check
```

3. Створити `deny.toml` з базовою конфігурацією:
```toml
[advisories]
vulnerability = "deny"
unmaintained = "warn"

[licenses]
allow = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib"]

[bans]
multiple-versions = "warn"
```

---

## Backlog (без прив'язки до релізу)

### TASK-014: Локалізація констант та port descriptions

**Пріоритет:** 🟢 Низький | **Оцінка:** 1-2 год | **Область:** i18n  
**Файли:** `rustconn/src/dialogs/widgets.rs`, `rustconn/src/dialogs/connection/general_tab.rs`

**Поточний код (widgets.rs:45-48):**
```rust
pub const ROOT_GROUP: &str = "(Root)";
pub const NONE: &str = "(None)";
pub const NO_KEYS_LOADED: &str = "(No keys loaded)";
```

**Виправлення — замінити const на функції:**
```rust
pub fn root_group() -> String { i18n("(Root)") }
pub fn none_label() -> String { i18n("(None)") }
pub fn no_keys_loaded() -> String { i18n("(No keys loaded)") }
```

**Port descriptions (general_tab.rs:455-490):**
```rust
// Поточний:
"Well-Known" | "Registered" | "Dynamic"
// Виправлення:
i18n("Well-Known") | i18n("Registered") | i18n("Dynamic")
```

Після змін — `po/update-pot.sh` для оновлення `.pot`.

---

### TASK-015: Desktop entry переклади

**Пріоритет:** 🟢 Низький | **Оцінка:** 30 хв | **Область:** i18n  
**Файл:** `rustconn/assets/io.github.totoshko88.RustConn.desktop`

**Поточний:**
```ini
Comment=Manage remote connections easily
```

**Додати:**
```ini
Comment=Manage remote connections easily
Comment[uk]=Зручне керування віддаленими підключеннями
Comment[de]=Fernverbindungen einfach verwalten
Comment[fr]=Gérer facilement les connexions distantes
Comment[es]=Gestionar conexiones remotas fácilmente
Comment[cs]=Snadná správa vzdálených připojení
```

---

### TASK-016: Accessible label для Command Palette list

**Пріоритет:** 🟢 Низький | **Оцінка:** 15 хв | **Область:** Accessibility  
**Файл:** `rustconn/src/dialogs/command_palette.rs`

**Що зробити:**
```rust
let list_box = ListBox::builder()./* ... */.build();
list_box.update_property(&[
    gtk4::accessible::Property::Label(&i18n("Search results"))
]);
```

---

### TASK-017: Звузити tokio features

**Пріоритет:** 🟢 Низький | **Оцінка:** 1 год | **Область:** Build / DX  
**Файл:** `Cargo.toml`

**Поточний:**
```toml
tokio = { version = "1", features = ["full"] }
```

**Виправлення:**
```toml
tokio = { version = "1", features = ["rt-multi-thread", "sync", "time", "io-util", "fs", "macros", "process"] }
```

Після зміни — `cargo build --all-targets && cargo test` для перевірки що всі потрібні features включені.

---

### TASK-018: Документувати модель загроз

**Пріоритет:** 🟢 Низький | **Оцінка:** 2-4 год | **Область:** Security / Docs  
**Файл:** `SECURITY.md`

**Що додати:**
- Модель загроз: хто атакує, які активи захищаються, які вектори
- Обґрунтування Argon2id параметрів (16 MiB, 2 ітерації — для machine-key, не для user passwords)
- Обґрунтування `--device=all` у Flatpak (serial port access)
- Рекомендації для користувачів: SSH tunnel для VNC, SPICE TLS налаштування

---

### TASK-019: Опціональне шифрування записів сесій

**Пріоритет:** 🟢 Низький | **Оцінка:** 4-8 год | **Область:** Security  
**Файли:** `rustconn-core/src/session/recording.rs`, `rustconn/src/dialogs/settings/`

**Проблема:**  
Session recordings зберігаються як plain text і можуть містити чутливу інформацію.

**Що зробити:**
1. Додати опцію в Settings → Session → "Encrypt session recordings"
2. Використати існуючий `encrypt_credential()` / `decrypt_credential()` для шифрування файлів записів
3. Зашифровані файли отримують розширення `.enc` замість `.log`
4. Log Viewer автоматично дешифрує при відкритті

---

### TASK-020: Видалити XOR-шифр повністю (v0.12)

**Пріоритет:** 🟢 Низький | **Оцінка:** 1 год | **Область:** Security  
**Залежність:** TASK-006 (попередження в v0.11)  
**Файл:** `rustconn-core/src/config/settings.rs`

**Що зробити:**
1. Видалити `xor_cipher_legacy()` метод
2. У `decrypt_credential()` — повертати помилку для даних без magic header `RCSC`:
```rust
fn decrypt_credential(data: &[u8], machine_key: &[u8]) -> Result<Vec<u8>, String> {
    if data.len() >= SETTINGS_HEADER_LEN && data[..4] == *SETTINGS_CRYPTO_MAGIC {
        decrypt_credential_aes(data, machine_key)
    } else {
        Err("Legacy XOR-encrypted credentials detected. \
             Please re-enter credentials — XOR encryption was removed in v0.12 \
             for security reasons.".to_string())
    }
}
```
3. Задокументувати breaking change в CHANGELOG та User Guide.

---

### TASK-021: Sidebar UX — pill-фільтри, animated revealer, ToolbarView

**Пріоритет:** 🟠 Високий | **Оцінка:** 3-4 год | **Область:** UX / GNOME HIG  
**Файли:** `rustconn/src/sidebar/mod.rs`, `rustconn/src/sidebar/filter.rs`, `rustconn/src/sidebar_ui.rs`, `rustconn/assets/style.css`

**Проблема:**  
Візуальна неконсистентність між сайдбаром і рештою інтерфейсу:
1. Панель фільтрів протоколів — ряд дрібних іконок без підписів, без чіткого візуального зв'язку з пошуком, без анімації появи/зникнення
2. Нижній тулбар — плоский `GtkBox` з іконками без візуального розділення від списку з'єднань (немає тіні/бордера як у GNOME Files)
3. Фільтри з'являються/зникають різко (`set_visible`) замість плавної анімації

**Що зробити:**

#### Крок 1: Animated Revealer для фільтрів

Замінити `filter_box.set_visible()` на `gtk4::Revealer` з анімацією:

```rust
// rustconn/src/sidebar/mod.rs — в конструкторі, замість container.append(&filter_box):
let filter_revealer = gtk4::Revealer::builder()
    .transition_type(gtk4::RevealerTransitionType::SlideDown)
    .transition_duration(200)
    .reveal_child(false) // hidden by default (settings control initial state)
    .child(&filter_box)
    .build();
container.append(&filter_revealer);
```

Оновити `set_filter_visible()` та `is_filter_visible()`:
```rust
pub fn set_filter_visible(&self, visible: bool) {
    self.filter_revealer.set_reveal_child(visible);
    if !visible {
        // existing cleanup logic...
    }
}

pub fn is_filter_visible(&self) -> bool {
    self.filter_revealer.reveals_child()
}
```

Замінити поле `filter_box: GtkBox` на `filter_revealer: gtk4::Revealer` в struct `ConnectionSidebar`.

#### Крок 2: Pill-кнопки для фільтрів

Змінити `create_filter_button()` в `filter.rs` — додати текстовий label поруч з іконкою:

```rust
pub fn create_filter_button(protocol: &str, icon_name: &str, tooltip: &str) -> Button {
    let button = Button::new();
    let content_box = GtkBox::new(Orientation::Horizontal, 4);
    let icon = gtk4::Image::from_icon_name(icon_name);
    icon.set_pixel_size(14);
    content_box.append(&icon);
    let label = gtk4::Label::new(Some(protocol));
    label.add_css_class("caption");
    content_box.append(&label);
    button.set_child(Some(&content_box));
    button.set_tooltip_text(Some(tooltip));
    button.add_css_class("pill");
    button.add_css_class("filter-button");
    // ... accessible label як раніше
    button
}
```

CSS для pill-стилю (`style.css`):
```css
/* Pill-style filter buttons */
.filter-button.pill {
    border-radius: 999px;
    padding: 2px 8px;
    min-height: 24px;
}
```

#### Крок 3: Separator між фільтрами і списком

```rust
// Після filter_revealer, перед overlay:
let separator = gtk4::Separator::new(Orientation::Horizontal);
separator.add_css_class("spacer");
container.append(&separator);
```

#### Крок 4: ToolbarView для нижнього тулбару

Замінити пряме `container.append(&bottom_toolbar)` на `adw::ToolbarView`:

```rust
// rustconn/src/sidebar/mod.rs — замість:
//   container.append(&overlay);
//   container.append(&bottom_toolbar);
// Використати:
let toolbar_view = adw::ToolbarView::new();
toolbar_view.set_content(Some(&overlay));
toolbar_view.add_bottom_bar(&bottom_toolbar);
toolbar_view.set_bottom_bar_style(adw::ToolbarStyle::Raised);
toolbar_view.set_vexpand(true);
container.append(&toolbar_view);
```

Це дасть нативну тінь зверху тулбару як у GNOME Files/Nautilus.

**Зміни в sidebar_ui.rs:**
- Видалити `margin_top: 6` з `create_sidebar_bottom_toolbar()` — `ToolbarView` сам додає відступ
- Додати `add_css_class("toolbar")` на контейнер тулбару для GNOME HIG стилю

**Зміни в struct `ConnectionSidebar`:**
- Замінити `filter_box: GtkBox` → `filter_revealer: gtk4::Revealer`
- Зберегти `filter_box` як приватне поле якщо потрібен доступ до дочірніх кнопок (або отримувати через `revealer.child()`)

**Порядок виконання:**
1. Крок 1 (Revealer) — найменш ризикований, чисто структурна зміна
2. Крок 3 (Separator) — один рядок
3. Крок 4 (ToolbarView) — потребує перевірки що overlay vexpand працює правильно
4. Крок 2 (Pill-кнопки) — найбільш візуально помітна зміна, потребує ручного тестування ширини

**Ризик:** Середній — зміни чисто візуальні, але потребують ручного тестування:
- Перевірити що revealer анімація не ламає layout при швидкому toggle
- Перевірити що ToolbarView не з'їдає простір scrolled window
- Перевірити pill-кнопки при вузькому сайдбарі (360px мінімум) — WrapBox має переносити
- Перевірити що drag-and-drop overlay працює з ToolbarView

**Тестування:** Ручне — `cargo run -p rustconn`, перевірити:
- [ ] Фільтри плавно з'являються/зникають при натисканні toggle
- [ ] Pill-кнопки показують іконку + назву протоколу
- [ ] Pill-кнопки переносяться на новий рядок при вузькому сайдбарі (adw-1-7)
- [ ] Нижній тулбар має тінь зверху (raised style)
- [ ] Separator видимий між фільтрами і списком
- [ ] Drag-and-drop працює як раніше
- [ ] Scroll списку з'єднань займає весь доступний простір
- [ ] Group operations mode працює як раніше

---

## Зведена таблиця

| # | Задача | Релiз | Пріоритет | Оцінка | Область |
|---|--------|-------|-----------|--------|---------|
| 001 | Видалити слабкий fallback машинного ключа | v0.10.21 | 🔴 | 2-4 год | Security |
| 002 | Права 0600 на конфігурацію | v0.10.21 | 🔴 | 1-2 год | Security |
| 003 | Локалізувати accessible labels sidebar | v0.10.21 | 🔴 | 30 хв | A11Y / i18n |
| 004 | Міграція General tab на adw:: widgets | v0.11.0 | 🟠 | 8-16 год | UX / HIG |
| 005 | Посилити валідацію automation tasks | v0.11.0 | 🟠 | 4-6 год | Security |
| 006 | Попередження XOR-шифру + план видалення | v0.11.0 | 🟠 | 2-3 год | Security |
| 007 | Структуризувати RDP connection state | v0.11.0 | 🟠 | 2-3 год | Architecture |
| 008 | Синхронізувати документацію (with_state) | v0.11.0 | 🟠 | 2-4 год | Architecture |
| 009 | Адаптивність діалогу підключення | v0.12.0 | 🟡 | 4-6 год | UX / Responsive |
| 010 | VNC security warnings | v0.12.0 | 🟡 | 4-8 год | Security / UX |
| 011 | Accessible relations у widget builders | v0.12.0 | 🟡 | 2-3 год | A11Y |
| 012 | Runtime warning для block_on_async | v0.11.0 | 🟡 | 1 год | Architecture |
| 013 | cargo audit у CI | v0.12.0 | 🟡 | 2-3 год | Security / CI |
| 014 | Локалізація констант та port descriptions | v0.11.0 | 🟢 | 1-2 год | i18n |
| 015 | Desktop entry переклади | v0.11.0 | 🟢 | 30 хв | i18n |
| 016 | Accessible label Command Palette list | v0.11.0 | 🟢 | 15 хв | A11Y |
| 017 | Звузити tokio features | Backlog | 🟢 | 1 год | Build / DX |
| 018 | Документувати модель загроз | Backlog | 🟢 | 2-4 год | Security / Docs |
| 019 | Шифрування записів сесій | Backlog | 🟢 | 4-8 год | Security |
| 020 | Видалити XOR-шифр повністю | v0.12.0 | 🟢 | 1 год | Security |
| 021 | Sidebar UX: pill-фільтри, revealer, ToolbarView | v0.11.0 | 🟠 | 3-4 год | UX / HIG |
