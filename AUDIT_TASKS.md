# RustConn Audit Tasks

Результат аудиту кодової бази. Таски відсортовані за пріоритетом.
Кожен таск — самодостатній і може бути виконаний окремо.

---

## Phase 1: Security Hardening (Critical)

### TASK-01: Bitwarden session key → SecretString

**Пріоритет:** Critical
**Домен:** Security
**Файли:** `rustconn-core/src/secret/bitwarden.rs`

**Проблема:**
`BW_SESSION_STORE` (рядок 62) — це `RwLock<Option<String>>`. Bitwarden session key дає повний доступ до vault і зберігається в пам'яті без zeroization. Порушує правило проєкту "всі credentials мають використовувати SecretString".

**Рішення:**
1. Змінити `BW_SESSION_STORE` на `RwLock<Option<SecretString>>`
2. `set_session_key` — приймати `SecretString` замість `&str`
3. `get_session_key` — повертати `Option<SecretString>`
4. `build_command` — використовувати `expose_secret()` тільки при передачі `--session` аргументу до `bw` CLI
5. `unlock_vault` / `unlock_vault_sync` — повертати `SecretString`
6. Оновити всі call sites в GUI layer

**Перевірка:**
```bash
cargo clippy --all-targets
cargo test -p rustconn-core
```

---

### TASK-02: Restrictive file permissions для config файлів

**Пріоритет:** Critical
**Домен:** Security
**Файли:** `rustconn-core/src/config/manager.rs`

**Проблема:**
`save_toml_file` (рядок 493) і `save_toml_file_async` (рядок 509) записують файли з дефолтними umask permissions (зазвичай 0644). Config файли містять hostnames, usernames, port forwards, SSH key paths — на multi-user системах це world-readable.

**Рішення:**
1. В `save_toml_file` після `fs::write` додати:
```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .map_err(|e| ConfigError::Write(
            format!("Failed to set permissions on {}: {}", path.display(), e)
        ))?;
}
```
2. В `save_toml_file_async` встановити permissions на temp файл перед rename
3. В `ensure_config_dir` встановити 0o700 на директорію
4. Перевірити `document` module — `encrypt_document` write path

**Перевірка:**
```bash
cargo test -p rustconn-core
ls -la ~/.config/rustconn/  # перевірити permissions після запуску
```

---

### TASK-03: SSH monitoring — прибрати StrictHostKeyChecking=no

**Пріоритет:** Critical
**Домен:** Security
**Файли:** `rustconn-core/src/monitoring/ssh_exec.rs`, `rustconn-core/src/monitoring/settings.rs`

**Проблема:**
Рядок 74 — `cmd.arg("-o").arg("StrictHostKeyChecking=no")` безумовно вимикає перевірку host key для ВСІХ monitoring SSH з'єднань. Це робить кожне monitoring з'єднання вразливим до MITM атак без жодного повідомлення користувачу.

**Рішення:**
1. Замінити `StrictHostKeyChecking=no` на `StrictHostKeyChecking=accept-new` (доступно з OpenSSH 7.6 — приймає нові ключі, відхиляє змінені)
2. Додати поле `strict_host_key_checking: bool` (default `true`) в `MonitoringSettings`
3. Якщо `false` — використовувати `StrictHostKeyChecking=no` (explicit opt-out)
4. В Flatpak — `UserKnownHostsFile` вже вказує на writable path (рядок 77), це залишити

```rust
// Замінити рядок 74:
if strict_host_key_checking {
    cmd.arg("-o").arg("StrictHostKeyChecking=accept-new");
} else {
    cmd.arg("-o").arg("StrictHostKeyChecking=no");
}
```

**Перевірка:**
```bash
cargo test -p rustconn-core
cargo clippy --all-targets
```

---

### TASK-04: Session log sanitization — підключити SENSITIVE_PATTERNS

**Пріоритет:** Critical
**Домен:** Security
**Файли:** `rustconn-core/src/session/logger.rs`

**Проблема:**
`SENSITIVE_PATTERNS` (рядок 719) і `SENSITIVE_VALUE_PATTERNS` (рядок 754) визначені але позначені `#[allow(dead_code)]`. `sanitize_output` функція працює, але ці вбудовані патерни ніколи не завантажуються в `SanitizeConfig::default()`. Session logs можуть містити паролі введені на `sudo` промптах, API ключі, SSH passphrases.

**Рішення:**
1. В `SanitizeConfig::default()` заповнити `custom_patterns` з `SENSITIVE_PATTERNS`
2. Прибрати `#[allow(dead_code)]` атрибути
3. Оновити тести щоб перевірити що патерни активні

**Перевірка:**
```bash
cargo test -p rustconn-core
cargo clippy --all-targets
```

---

## Phase 2: Security & Packaging (High)

### TASK-05: Document encryption — V2 magic header

**Пріоритет:** High
**Домен:** Security
**Файли:** `rustconn-core/src/document/mod.rs`

**Проблема:**
Коментар на рядку 695 визнає: якщо перший байт legacy salt = 0, 1 або 2, він може бути помилково інтерпретований як strength byte нового формату. Ймовірність ~1.2% (3/256). Fallback працює, але це крихко і додає зайву латентність.

**Рішення:**
1. Ввести новий magic header `b"RCDB_EN2"` для нового формату
2. `encrypt_document` — використовувати `ENCRYPTED_MAGIC_V2`
3. `decrypt_document` — спочатку перевіряти V2, потім fallback на V1
4. Існуючі документи з старим magic продовжують працювати через legacy path
5. Додати property test для round-trip encryption/decryption обох форматів

**Перевірка:**
```bash
cargo test -p rustconn-core
cargo test -p rustconn-core --test property_tests
```

---

### TASK-06: Flatpak --device=all → scoped serial permissions

**Пріоритет:** High
**Домен:** Security / Packaging
**Файли:** `packaging/flatpak/io.github.totoshko88.RustConn.yml`, `packaging/flathub/io.github.totoshko88.RustConn.yml`

**Проблема:**
`--device=all` дає доступ до ВСІХ device nodes (`/dev/*`), включаючи GPU, USB, block devices. Використовується тільки для serial port доступу (picocom). Flathub рев'юери можуть це відхилити.

**Рішення:**
Замінити `--device=all` на scoped доступ:
```yaml
# Замість:
- --device=all
# Використати:
- --device=serial
```
Якщо `--device=serial` не підтримується runtime, використати:
```yaml
- --filesystem=/dev/ttyS*:rw
- --filesystem=/dev/ttyUSB*:rw
- --filesystem=/dev/ttyACM*:rw
```

**Перевірка:**
Тестувати serial з'єднання в Flatpak sandbox після зміни.

---

### TASK-07: Monitoring password → SecretString

**Пріоритет:** High
**Домен:** Security
**Файли:** `rustconn-core/src/monitoring/ssh_exec.rs`, call sites в GUI layer

**Проблема:**
`password: Option<String>` (рядок 32) — monitoring SSH пароль живе як plain `String` в captured state closure протягом всієї monitoring сесії без zeroization.

**Рішення:**
1. Змінити параметр на `Option<SecretString>`
2. Використовувати `expose_secret()` тільки при встановленні `SSHPASS` env var:
```rust
if let Some(ref pw) = password {
    cmd.env("SSHPASS", pw.expose_secret());
}
```
3. Оновити всі call sites в GUI layer щоб передавати `SecretString`

**Перевірка:**
```bash
cargo clippy --all-targets
cargo test
```

---

### TASK-08: Видалити sshpass залежність — нативний VTE password handling

**Пріоритет:** High
**Домен:** Security / Architecture
**Файли:**
- `rustconn-core/src/monitoring/ssh_exec.rs`
- `rustconn/src/window/protocols.rs`
- `packaging/flatpak/io.github.totoshko88.RustConn.yml`
- `packaging/flathub/io.github.totoshko88.RustConn.yml`

**Проблема:**
`sshpass` передає паролі через environment variables або file descriptors. Це зовнішня залежність яка:
- Вимагає окремого модуля в Flatpak manifest
- Передає пароль через `SSHPASS` env var (видимий в `/proc/PID/environ`)
- Не інтегрується з VTE terminal password prompts

**Рішення:**
Для інтерактивних SSH сесій (VTE terminal):
1. Використовувати VTE PTY для автоматичного введення паролю — моніторити output на "password:" prompt через `vte::Terminal::connect_commit` або `connect_contents_changed`
2. При виявленні password prompt — надіслати пароль через `vte::Terminal::feed_child()` з `SecretString`
3. Це нативний підхід який не потребує зовнішніх залежностей

Для monitoring SSH (non-interactive, `ssh_exec_factory`):
1. Використовувати `SSH_ASKPASS` з тимчасовим скриптом замість `sshpass`:
   - Створити temp файл з `#!/bin/sh\necho "$RUSTCONN_SSH_PASS"` (permissions 0700)
   - Встановити `SSH_ASKPASS` env var на цей скрипт
   - Встановити `SSH_ASKPASS_REQUIRE=force`
   - Видалити temp файл після завершення команди
2. Або використовувати `expect`-подібний підхід через PTY в tokio

Після реалізації:
1. Видалити `sshpass` модуль з обох Flatpak manifests
2. Видалити перевірку `sshpass` availability в `ssh_exec_factory`

**Перевірка:**
```bash
cargo test
# Тестувати password-based SSH з'єднання в Flatpak
# Тестувати monitoring з password auth
```

---

## Phase 3: Architecture & Crypto (Medium)

### TASK-09: RDP TLS — задокументувати trust-all certificate policy

**Пріоритет:** Medium
**Домен:** Security / Documentation
**Файли:** `rustconn-core/src/rdp_client/client/connection.rs`

**Проблема:**
`establish_connection` викликає `ironrdp_tls::upgrade` без certificate verification callback. Немає user-facing prompt для підтвердження сертифіката. Це еквівалент `xfreerdp /cert:ignore` але не задокументовано.

**Рішення:**
1. Додати doc-comment на `establish_connection` що пояснює:
   - IronRDP виконує TLS handshake але не валідує server certificate
   - Це очікувана поведінка для RDP (більшість RDP серверів використовують self-signed certs)
   - Еквівалент `/cert:ignore` в xfreerdp
2. Додати `tracing::warn!` при першому з'єднанні до нового хоста
3. В майбутньому — реалізувати TOFU (Trust On First Use) модель з збереженням fingerprint

**Перевірка:**
```bash
cargo clippy --all-targets
```

---

### TASK-10: Зменшити складність state.rs і window/mod.rs

**Пріоритет:** Medium
**Домен:** Architecture
**Файли:** `rustconn/src/state.rs` (3143 рядки), `rustconn/src/window/mod.rs` (5316 рядків)

**Проблема:**
God-objects. `state.rs` змішує connection CRUD, document management, cluster management, template management, history, clipboard, vault operations, credential resolution. `window/mod.rs` має ~80 методів на `MainWindow`.

**Рішення:**
Витягнути domain-specific facades з `AppState`:

**state.rs:**
- `VaultOperations` — рядки 2161–3143 (vault save/load/rename/delete/dispatch)
- `DocumentOperations` — рядки 1736–1827 (document CRUD)
- `ClusterOperations` — рядки 1836–1881 (cluster CRUD)
- `HistoryOperations` — рядки 1981–2066 (history recording/trimming)

**window/mod.rs:**
- Створити `window/actions/` директорію
- Перенести кожен `setup_*_actions` метод в окремий файл:
  - `window/actions/connection.rs`
  - `window/actions/terminal.rs`
  - `window/actions/navigation.rs`
  - `window/actions/snippet.rs`
  - `window/actions/cluster.rs`
  - `window/actions/template.rs`
  - `window/actions/document.rs`
  - `window/actions/split_view.rs`

**Підхід:** Один extraction per PR. Без поведінкових змін.

**Перевірка:**
```bash
cargo clippy --all-targets
cargo test
```

---

### TASK-11: Видалити dead read_import_file_async

**Пріоритет:** Medium
**Домен:** Architecture
**Файли:** `rustconn-core/src/import/traits.rs`

**Проблема:**
Функція `read_import_file_async` (рядок 75) — dead code з моменту створення. Всі importers використовують синхронне читання файлів.

**Рішення:**
Видалити функцію.

**Перевірка:**
```bash
cargo clippy --all-targets
```

---

### TASK-12: Backup/Restore — додати UI

**Пріоритет:** Medium
**Домен:** UX
**Файли:** `rustconn-core/src/config/manager.rs`, `rustconn/src/dialogs/settings/`

**Контекст:**
`ConfigManager::backup_to_archive` і `ConfigManager::restore_from_archive` — це функції для створення ZIP-архіву з усіх config файлів (connections, groups, snippets, clusters, templates, history, settings) і відновлення з такого архіву. Вони:
- Пакують всі відомі config файли (`BACKUP_FILES` список) в ZIP з deflate compression
- При restore — розпаковують тільки відомі файли (невідомі entries ігноруються)
- Логують операції через `tracing::info!`

Наразі ці функції доступні тільки через `rustconn-cli`. В GUI немає кнопок для backup/restore.

**Рішення:**
1. Додати "Backup" і "Restore" кнопки в Settings dialog (секція "Data")
2. Backup — відкрити `gtk::FileDialog` для вибору destination, викликати `backup_to_archive`, показати toast з кількістю файлів
3. Restore — відкрити `gtk::FileDialog` для вибору архіву, показати confirmation dialog ("This will overwrite current settings"), викликати `restore_from_archive`, перезавантажити state
4. Обгорнути strings в `gettext`

**Перевірка:**
```bash
cargo clippy --all-targets
cargo test
```

---

## Відповіді на питання аудиту

### Q1: RDP TLS certificate validation
→ Задокументувати як очікувану поведінку (TASK-09). IronRDP не валідує server certificate — це стандартна практика для RDP де більшість серверів використовують self-signed certs.

### Q2: Flatpak fallback-x11
→ **Уточнення потрібне від розробника:** `--socket=fallback-x11` в Flatpak manifest означає що X11 використовується ТІЛЬКИ коли Wayland недоступний. `SafeFreeRdpLauncher::with_x11_fallback()` існує для зовнішнього FreeRDP який може не підтримувати Wayland. Чи варто залишити X11 fallback для сумісності з зовнішнім FreeRDP на X11 сесіях, чи видалити і вимагати тільки Wayland-native `wlfreerdp`?

### Q3: sshpass в Flatpak
→ Відмовитись від sshpass залежності (TASK-08). Реалізувати нативний VTE password handling через PTY та SSH_ASKPASS для monitoring.

### Q4: Document encryption Argon2 parameters
→ Параметри адекватні: Standard = 64MB RAM / 3 iterations / 4 parallelism (Argon2id). High = 128MB/4/8. Maximum = 256MB/6/8. Це відповідає OWASP рекомендаціям.

### Q5: Backup/Restore
→ Описано в TASK-12. Функціонал пакує всі config файли (connections.toml, groups.toml, snippets.toml, clusters.toml, templates.toml, history.toml, settings.toml, trash.toml) в ZIP архів і може відновити з нього. Наразі доступний тільки через CLI.
