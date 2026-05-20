# RustConn — Audit Backlog (post-hotfix)

> Generated from the 0.14.1 audit (2026-05-19).
> Hotfix items live in CHANGELOG `[Unreleased]`; everything below is **out of hotfix**
> and scheduled for 0.15.x or later.

Severity legend:
- **blocker** — feature broken / data loss / declared in README but missing
- **major** — significant deviation from HIG, missing CLI parity, design rule violation
- **minor** — code smell or limited UX impact
- **nit** — cosmetic / docs

---

## ARCH-1 [major] Винести pre-connect probe-bypass логіку в `Connection`

Дублюється в 5+ місцях GUI. Регресії на кшталт 0.14.1 RDP Gateway чекають своєї черги.
SPICE через `proxy` зараз НЕ покритий — той самий патерн.

Додати у `rustconn-core/src/models/connection.rs`:

    impl Connection {
        pub fn bypasses_direct_probe(&self) -> bool {
            match &self.protocol_config {
                ProtocolConfig::Ssh(c) | ProtocolConfig::Sftp(c) => c.jump_host_id.is_some(),
                ProtocolConfig::Rdp(c) => c.jump_host_id.is_some() || c.gateway.is_some(),
                ProtocolConfig::Vnc(c) => c.jump_host_id.is_some(),
                ProtocolConfig::Spice(c) => c.jump_host_id.is_some() || c.proxy.is_some(),
                ProtocolConfig::ZeroTrust(_) | ProtocolConfig::Web(_) => true,
                _ => false,
            }
        }
        pub fn should_pre_connect_check(&self, settings: &AppSettings) -> bool {
            settings.connection.pre_connect_port_check
                && !self.skip_port_check
                && !self.bypasses_direct_probe()
        }
    }

Замінити 5+ inline-перевірок на виклик `conn.should_pre_connect_check(settings)`.
Додати property tests `rustconn-core/tests/properties/connection_probe.rs`.

Імпакт: один спосіб правди, неможливість регресій. Низький ризик.

---

## ARCH-2 [major] File-locking конфігу

`rustconn-core/src/config/manager.rs` зберігає атомарно, але без advisory-lock.
Паралельні `rustconn` GUI + `rustconn-cli add` дають lost-update.

У `ConfigManager::new`:

    let lock_path = config_dir.join(".lock");
    let lock_file = std::fs::OpenOptions::new()
        .create(true).read(true).write(true).open(&lock_path)?;
    fs2::FileExt::try_lock_exclusive(&lock_file)
        .or_else(|_| {
            eprintln!("Waiting for another rustconn instance...");
            fs2::FileExt::lock_exclusive(&lock_file)
        })?;
    self.lock_file = Some(lock_file);

Тести: запуск двох ConfigManager у паралельних потоках, перевірка що другий чекає.

Імпакт: усуває lost-update. CLI стає safe для cron/scripts.

---

## ARCH-3 [major] window_mode — UI vs реальність

Поле `Connection.window_mode` обробляється лише у `window/rdp_vnc.rs` для RDP/VNC.
Для SSH/SPICE/Telnet/Mosh/K8s/Serial/ZeroTrust значення мовчки ігнорується.

Варіант A (рекомендований): прибрати поле з UI повсюдно (або сховати для не-RDP/VNC),
лишити в моделі для backward compatibility.
Варіант B: поширити обробку на всі протоколи (External -> окремий adw::ApplicationWindow,
Fullscreen -> gtk::Window::fullscreen()).

Імпакт: усуває false expectations. Варіант A безпечний.

---

## ARCH-4 [major] Перевести `add_key()` на `&SecretString`

`rustconn-core/src/ssh_agent/mod.rs:500` приймає `passphrase: Option<&str>`.
Public API change -> bump до 0.15.0.

Нова сигнатура:

    pub fn add_key(
        &self,
        key_path: &Path,
        passphrase: Option<&SecretString>,
    ) -> AgentResult<()> {
        if let Some(pass) = passphrase {
            use secrecy::ExposeSecret;
            let escaped = Zeroizing::new(pass.expose_secret().replace('\'', "'\\''"));
            // ... fs::write нижче
        }
        ...
    }

Виклики (`ssh_agent_tab.rs`, `ssh_agent_dialog.rs`) — обгортати GString у SecretString.

Імпакт: усуває порушення правила "passwords/keys -> SecretString".

---

## ARCH-5 [major] Декомпозиція файлів >2000 рядків

11 кандидатів. По одному файлу за раз, кожен PR — pure-move без логіки.

| File | Lines | Подальша структура |
|------|-------|-------------------|
| dialogs/connection/dialog.rs | 7176 | per-tab modules уже існують частково; винести validate(), build_connection(), populate_from() у dialog/{builders,validation,persistence}.rs |
| window/mod.rs | 4000 | actions -> window/actions/{connection,group,session,sync,view}.rs |
| terminal/mod.rs | 3803 | playback, recording, snippets — у вже існуючі submodules |
| dialogs/template.rs | 3511 | builtin-templates у template/builtin.rs |
| window/edit_dialogs.rs | 3213 | Edit Group -> window/edit_dialogs/group.rs з PreferencesDialog |
| window/protocols.rs | 3099 | per-protocol launch logic у window/protocols/{...}.rs |
| rustconn-core/src/models/protocol.rs | 2967 | per-protocol struct'и в окремі файли |
| dialogs/settings/secrets_tab.rs | 2533 | per-backend модулі |
| dialogs/import.rs | 2518 | source-detect та per-source UI у import/sources.rs |
| state.rs | 2501 | sub-state структури state/{connections,sessions,sync}.rs |
| embedded_vnc.rs | 2055 | UI у embedded_vnc/ui.rs (об'єднати з embedded_vnc_ui.rs) |

Імпакт: час компіляції падає, code review простіший, ризик регресій низький при чистому move.

---

## SEC-1 [major] CLI `--password` -> Zeroizing одразу

`rustconn-cli/src/cli.rs:1113` — `password: Option<String>`. Між clap і `SecretString::from`
пароль існує як plain heap String + видимий у /proc/<pid>/cmdline.

Реалізація: одразу обгорнути у Zeroizing, краще прибрати --password зовсім, або додати
--password-stdin як у `pass insert -m`.

---

## SEC-2 [minor] Askpass на CoW-FS

`rustconn-core/src/ssh_agent/mod.rs:519`. Перезапис нулями ненадійний на APFS/btrfs.
Альтернатива — pipe-на-stdin замість файлу, або memfd_create на Linux.

---

## CLI-1 [blocker] CLI add/update — додати поля Connection

CLI має ~30% паритету з GUI ConnectionDialog.

Хвиля 1 (загальні поля):

    #[arg(short, long)] tags: Option<String>,
    #[arg(short, long)] description: Option<String>,
    #[arg(short, long)] group: Option<String>,
    #[arg(long)] domain: Option<String>,
    #[arg(long)] window_mode: Option<String>,
    #[arg(long)] skip_port_check: bool,
    #[arg(long)] add_tag: Vec<String>,
    #[arg(long)] remove_tag: Vec<String>,

Хвиля 2 (SSH):

    #[arg(long)] x11_forwarding: bool,
    #[arg(long)] agent_forwarding: bool,
    #[arg(long)] compression: bool,
    #[arg(long)] startup_command: Option<String>,
    #[arg(long)] proxy_command: Option<String>,
    #[arg(long, value_name = "K=V")] ssh_option: Vec<String>,
    #[arg(long)] local_forward: Vec<String>,
    #[arg(long)] remote_forward: Vec<String>,
    #[arg(long)] dynamic_forward: Vec<String>,

Хвиля 2 (RDP):

    #[arg(long)] gateway: Option<String>,
    #[arg(long)] gateway_port: Option<u16>,
    #[arg(long)] gateway_username: Option<String>,
    #[arg(long)] remote_app_program: Option<String>,
    #[arg(long)] remote_app_args: Option<String>,
    #[arg(long)] remote_app_name: Option<String>,
    #[arg(long)] resolution: Option<String>,
    #[arg(long)] color_depth: Option<u8>,
    #[arg(long)] disable_nla: bool,
    #[arg(long)] keyboard_layout: Option<u32>,
    #[arg(long)] audio_redirect: bool,
    #[arg(long, value_name = "host:remote")] shared_folder: Vec<String>,

VNC/SPICE/MOSH/Serial — аналогічно.

Імпакт: повний паритет з GUI, headless management як обіцяно у README.

---

## CLI-2 [blocker] Команди верхнього рівня

Додати у Commands enum:

    History(HistoryCommands),       // list / clear / show <id>
    Pin { name: String },
    Unpin { name: String },
    Tag(TagCommands),               // add / remove / list
    Move { name: String, --group: String },
    WolConfig { name, --mac, --broadcast, --port, --wait },
    ClusterConnect { name },
    ClusterExec { name, --command },
    SyncSetMode { group, --mode, --file },
    Monitor(MonitorCommands),       // enable / disable / metrics
    SecretEx(SecretExCommands),     // list / rotate / test / unlock / lock

Файли: commands/{history,pin,tag,move,wol_config,cluster_exec,sync_mode,monitor,secret_ex}.rs.

Імпакт: повноцінний headless tool, паритетний з GUI для всіх CRUD.

---

## CLI-3 [major] Auto-detect imports

GUI має detect-mode для Asbru/Remmina. CLI приймає тільки file path.

    Import {
        #[arg(long, conflicts_with = "file")] auto: bool,
        file: Option<PathBuf>,
        ...
    }

Виклик `is_available()` -> `import_auto()` з ~/.config/asbru-cm/ та ~/.local/share/remmina/.

---

## CLI-4 [major] CSV options + import dry-run

    Export {
        #[arg(long, value_parser = ["comma", "semicolon", "tab"])]
        csv_delimiter: Option<String>,
        #[arg(long)] csv_fields: Option<String>,
    }
    Import {
        #[arg(long)] dry_run: bool,
    }

---

## UX-1 [major] Міграція великих діалогів на adw::Dialog

25 діалогів досі на adw::Window. High-impact:

connection/dialog.rs:1729, template.rs:127, import.rs:52, export.rs:93, cluster.rs:55,
snippet.rs:63, smart_folder.rs:66, variables.rs:61, recording.rs:76, tunnel.rs:44.

Pattern (як password.rs):

    let dialog = adw::Dialog::builder()
        .title(i18n("New Connection"))
        .content_width(600)
        .content_height(720)
        .build();
    let toolbar_view = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&clamped_content));
    dialog.set_child(Some(&toolbar_view));
    dialog.present(Some(parent_widget));

Імпакт: bottom-sheet на narrow, auto-close on Escape, drag-to-close.

---

## UX-2 [major] ConnectionDialog adaptive

Після UX-1 додати breakpoint + AdwClamp:

    let breakpoint = adw::Breakpoint::new(
        adw::BreakpointCondition::new_length(
            adw::BreakpointConditionLengthType::MaxWidth,
            500.0, adw::LengthUnit::Sp,
        ),
    );
    breakpoint.add_setter(&view_switcher_bar, "reveal", &true.to_value());
    dialog.add_breakpoint(breakpoint);

    let clamp = adw::Clamp::builder().maximum_size(600).build();
    clamp.set_child(Some(&main_box));

---

## UX-3 [major] Edit Group -> PreferencesDialog tabs

`window/edit_dialogs.rs:625-1351` пакує SSH+Sync+Automation+Dynamic+Description у один Box.
Розбити на adw::PreferencesDialog:

- Identity: name, icon, parent, description.
- SSH Inheritance: key path, auth method, ProxyJump, agent socket.
- Cloud Sync: sync_mode, sync_file, access_devices.
- Automation: expect_rules, post_login_scripts.
- Dynamic Folder: script, workdir, timeout, refresh_interval.

---

## UX-4 [major] Quick Connect history персистити

Зараз `window/types.rs:28-106` — Rc<RefCell<Vec<Entry>>>, рантайм-only.

Додати у settings:

    // rustconn-core/src/config/settings.rs
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quick_connect_history: Vec<QuickConnectHistoryEntry>,

Без секретів — лише protocol/host/port/username/last_used. Запис при додаванні в
`add_to_quick_connect_history` -> debounced save 1s.

---

## UX-5 [major] Wizard SecurityKey + fluid Advanced

1. У `dialogs/connection_wizard/auth_page.rs:91-101` додати CheckButton "Security Key (FIDO2)".
2. Замість close+open при OpenAdvanced — push сторінки advanced'а у тому ж NavigationView.

---

## UX-6 [minor] OK/Cancel pair у dialog_header()

`dialogs/widgets.rs:25-39` повертає start_btn (Cancel). Прибрати: adw::Dialog сам ловить Escape.

    pub fn dialog_header(end_label: &str) -> (adw::HeaderBar, Button) { ... }

Користувачі: password.rs:64, document.rs:73, 337, 562.

---

## UX-7 [minor] ✅ CheckButton-у-ActionRow -> AdwSwitchRow (done in 0.14.3)

25 toggles converted across `dialogs/settings/{ui_tab,terminal_tab,monitoring_tab,logging_tab}.rs`.
Pattern: `CheckButton` + `AdwActionRow` → `AdwSwitchRow`. Signal: `connect_toggled` → `connect_active_notify`.

**UX-7b ✅** (done in 0.14.3): `secrets_tab.rs` 4 backend pairs of "Save password" + "Save to keyring"
CheckButtons collapsed into a single `AdwComboRow` with three canonical choices ("Don't save" /
"Encrypted file (machine-specific)" / "System keyring (recommended)"). The hand-rolled mutual-exclusion
code is gone; `secret-tool` availability is enforced inside `make_storage_combo()`. Persistence is
unchanged: `CredentialStorage` enum + `*_storage()` / `set_*_storage()` helpers on `SecretSettings` map
to/from the legacy `*_password_encrypted` + `*_save_to_keyring` fields, so old configs round-trip
without a migration step. Property tests in
`rustconn-core/tests/properties/credential_storage_tests.rs` cover the mapping table, the round-trip,
the legacy-conflict resolution, and the field-clearing semantics for "None" / "SystemKeyring".

Affected backends: KeePassXC, Bitwarden, 1Password, Passbolt.

---

## UX-8 [minor] Color scheme: AdwToggleGroup замість 3 ToggleButton у Box

`dialogs/settings/ui_tab.rs:40-89`. На libadwaita 1.7+ -> AdwToggleGroup.
Якщо <1.7 -> AdwComboRow з 3 варіантами.

---

## UX-9 [minor] Auto-reconnect банер — attempt N/M

`terminal/mod.rs:2143-2200`:

    state.banner.set_title(&i18n_f(
        "Auto-reconnecting (attempt {}/{})",
        &[&attempt.to_string(), &max.to_string()]
    ));

---

## UX-10 [minor] external_window.rs -> libadwaita

`rustconn/src/external_window.rs:7, 50` — gtk::ApplicationWindow + gtk::HeaderBar.
Замінити на adw::ApplicationWindow + adw::ToolbarView + adw::HeaderBar.

---

## UX-11 [minor] Icon-buttons без accessible::Property::Label

Точкові випадки (steering rule violation):

- window/snippets.rs:257-272 — Execute/Edit/Delete buttons.
- dialogs/connection/dialog.rs:1021-1024 — ssh_key_browse_btn.

Шаблон:

    btn.update_property(&[gtk4::accessible::Property::Label(&i18n("Browse for SSH key"))]);

---

## UX-12 [nit] Search-syntax help popover локалізувати

`sidebar/search.rs:11-42`. Замінити hardcoded EN markup на i18n_f() з {}-плейсхолдерами,
використати add_css_class("heading") замість <b>.

---

## UX-13 [nit] ui.rs:77 stale tooltip

`rustconn/src/window/ui.rs:77`: tooltip "New Group (Ctrl+Shift+N)", але реальний accel
"Ctrl+Shift+G" (`keybindings.rs:171`). Виправити tooltip або винести через keybindings_settings.

---

## TEST-1 [minor] Property tests на регресуючі сценарії

Додати у `rustconn-core/tests/properties/`:

- `csv_port_overflow.rs` — генерувати CSV з port > u16::MAX, очікувати Err.
- `connection_probe.rs` — після ARCH-1, для випадково згенерованого Connection.
- `concurrent_save.rs` — два ConfigManager одночасно, після ARCH-2.
- `sync_path_traversal.rs` — fuzz sync_file з .., абсолютними шляхами.

---

## DOC-1 [minor] Оновити docs/CLI_REFERENCE.md

Версія у заголовку "0.13.6" — застаріла. Оновити після кожного релізу.
Додати розділи про поля які впроваджуються через CLI-1/CLI-2.

---

## CODE-1 [minor] Усунути дрібну дубльованість

- `vault_ops.rs:482-493` — collect_descendant_groups дублює core (O(n²) замість O(n)).
- `cli_download/extract.rs::find_binary_*` vs `dialogs/settings/clients_tab.rs:613` — об'єднати.
- `commands/connect.rs:69-110` ZeroTrust build_command — делегувати у ProtocolRegistry.

---

## Roadmap suggestion

| Release | Items |
|---------|-------|
| 0.14.2 (hotfix) | див. CHANGELOG `[Unreleased]` — ARCH-1 ✅, UX-9 ✅, UX-12 ✅, DOC-1 ✅, TEST-1 (connection_probe) ✅ |
| 0.14.3 (UI polish) | UX-7 ✅ (25 settings toggles → AdwSwitchRow) |
| 0.15.0 | ARCH-2, ARCH-4 (semver-break), CLI-1 wave 1, UX-4, UX-13 ✅ (done in 0.14.2) |
| 0.15.x | UX-1 (high-impact dialogs), UX-2, UX-5, CLI-1 wave 2, CLI-2 (history+pin+tag) |
| 0.16.0 | ARCH-3 (decision), ARCH-5 (decomposition by file), решта CLI-2 |
| 0.16.x | UX-3, UX-6, UX-8, UX-10, UX-11, CLI-3, CLI-4, TEST-1 (решта), CODE-1 |
| 0.17.0 | UX-1 решта (low-impact dialogs) |
