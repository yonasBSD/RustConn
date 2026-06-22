# План релізу RustConn 0.17.0

> Статус: чернетка плану за результатами повного аудиту кодової бази (безпека,
> продуктивність, best practices, правила проєкту, GNOME HIG, незавершений
> функціонал, мертвий код).
> Поточна версія: **0.16.13** → ціль: **0.17.0**.
> Дата складання: 2026-06-22.

## Методологія аудиту

Аудит виконано чотирма паралельними напрямами (безпека, продуктивність,
правила/мертвий код, HIG/a11y/i18n) з подальшою **критичною перевіркою** кожної
ключової знахідки безпосередньо в коді. Нижче — лише те, що пройшло перевірку.
Розділ «Свідомо відкинуто» фіксує знахідки, які НЕ йдуть у реліз, і причину.

Загальний висновок аудиту: кодова база у дуже доброму стані. Критичних дірок
безпеки немає (`unsafe` ізольовано в `rustconn-pty-sys`, секрети в `SecretString`,
AES-256-GCM + Argon2id, спавн через argv без shell). HIG-відповідність висока
(всі діалоги на `adw::AlertDialog`, i18n чистий, шорткати на місці). Тому 0.17.0 —
це реліз **загартування**: точкові виправлення безпеки, продуктивності та
прибирання технічного боргу, а не велика нова функціональність (YAGNI).

---

## P0 — Безпека та цільові для 0.17

### 0.1. kubectl: ін'єкція через `sh -c` (підтверджено)

**Файл:** `rustconn/src/window/protocols.rs:852-855`
**Проблема:** `command.join(" ")` склеює аргументи (namespace/pod/container) і
виконує їх через `sh -c` без екранування. Поле з shell-метасимволами
(`;`, `$()`, backtick) → локальна ін'єкція команд. Особливо небезпечно для
**імпортованих** (недовірених) конфігів. SSH/RDP/VNC/telnet/serial/mosh — безпечні
(argv через VTE), тож kubectl тут — виняток, що випадає з безпечного патерну.

**Поточний код:**
```rust
let spawn_cmd = command.join(" ");
let wrapped = rustconn_core::flatpak::wrap_host_command(&spawn_cmd);
let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
notebook.spawn_command(session_id, &[&shell, "-c", &wrapped], None, None, None);
```

**Рішення:** поза Flatpak — спавнити argv напряму (як mosh нижче в тому ж файлі).
Усередині Flatpak, де потрібна обгортка `flatpak-spawn --host`, екранувати **кожен**
елемент через наявний `shell_escape`, а не склеювати сирим `join(" ")`.

```rust
// Поза Flatpak: прямий argv, без shell-інтерпретації.
if !rustconn_core::flatpak::is_sandboxed() {
    let argv: Vec<&str> = command.iter().map(String::as_str).collect();
    notebook.spawn_command(session_id, &argv, None, None, None);
} else {
    // У Flatpak потрібен sh -c для flatpak-spawn-обгортки —
    // екрануємо КОЖЕН елемент, а не склеюємо сирим join(" ").
    let escaped = command
        .iter()
        .map(|a| rustconn_core::shell_escape::escape_path(a))
        .collect::<Vec<_>>()
        .join(" ");
    let wrapped = rustconn_core::flatpak::wrap_host_command(&escaped);
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    notebook.spawn_command(session_id, &[&shell, "-c", &wrapped], None, None, None);
}
```
**Зусилля:** S. **Те саме перевірити для Zero-Trust Generic поза Flatpak** (`protocols.rs:1316-1338`).

### 0.2. ironrdp: переоцінити catch_unwind навколо connect (TODO(0.17))

**Файл:** `rustconn-core/src/rdp_client/client/connection.rs:248`
**Проблема:** код містить явну позначку `TODO(0.17): re-evaluate on the next
ironrdp bump (>0.16)`. У 0.16.13 ми вже піднялися на ironrdp 0.16. Потрібно
перевірити, чи upstream усунув паніки на некоректних PDU (`connect_finalize`),
і якщо так — прибрати обгортку `catch_unwind` (deletion over addition). Якщо ні —
лишити та оновити коментар із актуальним станом і посиланням на upstream-issue.

**Дія:** перевірити changelog/issues ironrdp 0.16, прийняти рішення keep/remove,
оновити коментар. **Зусилля:** S (дослідження) + S (код).

### 0.3. Passbolt: passphrase в аргументах командного рядка

**Файл:** `rustconn-core/src/secret/passbolt.rs:143`
**Проблема:** `cmd.arg("--userPassword").arg(passphrase.expose_secret())` — секрет
видно в `/proc/<pid>/cmdline` будь-якому процесу того ж UID на час виконання.
Обмеження upstream `go-passbolt-cli` (немає stdin-вводу).

**Рішення для 0.17 (без коду — документування + дослідження):**
1. Зафіксувати в `SECURITY.md` як **Known Issue** з описом моделі загроз.
2. Перевірити, чи `go-passbolt-cli` додав stdin/env-input; якщо так — перейти на
   env-var (як уже зроблено для SSH `ASKPASS`) або тимчасовий файл 0600.
**Зусилля:** S.

### 0.4. Документувати модель загроз machine-key шифрування

**Файл:** `rustconn-core/src/config/settings.rs:1063` (`get_machine_key`)
**Проблема:** `*_encrypted` поля шифруються ключем, що лежить поруч
(`~/.local/share/rustconn/.machine-key`). Це захист від випадкового перегляду
диска/бекапів, але **не** від атакника з доступом на читання як той самий
користувач. Ризик — хибне відчуття безпеки.

**Рішення:** у `docs/` (напр. `SECURITY.md` або `USER_GUIDE.md`) чітко описати, що
для справжніх секретів слід використовувати keyring/vault-бекенди (вони вже
підтримуються), а machine-key шифрування — це обфускація «at rest».
**Зусилля:** S (лише документація).

---

## P1 — Продуктивність (підтверджено, реальний вплив)

### 1.1. Пре-парсинг hex-кольорів у движку підсвічування (найбільша віддача)

**Файли:** `rustconn-core/src/highlight.rs`, `rustconn/src/terminal/highlight_overlay.rs:184,195`
**Проблема:** `parse_hex_color()` викликається на **кожен збіг кожного рядка при
кожному repaint** терміналу — найгарячіший шлях рендеру. Кольори зберігаються як
`Option<String>` і парсяться з нуля щоразу. Додатково `find_matches` клонує
`Option<String>` кольори на кожен збіг.

**Рішення:** парсити кольори **один раз** під час `CompiledHighlightRules::compile`
і зберігати як `Option<Rgb>` (де `type Rgb = (f64, f64, f64)`). Тоді overlay просто
читає готові значення.

```rust
// rustconn-core/src/highlight.rs
/// Pre-parsed RGB у діапазоні 0.0..=1.0 для прямого передавання в cairo.
type Rgb = (f64, f64, f64);

struct CompiledRule {
    regex: Regex,
    name: String,
    pattern: String,
    foreground_rgb: Option<Rgb>,   // було: foreground_color: Option<String>
    background_rgb: Option<Rgb>,   // було: background_color: Option<String>
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighlightMatch {
    pub start: usize,
    pub end: usize,
    pub foreground_rgb: Option<Rgb>,
    pub background_rgb: Option<Rgb>,
}

// у compile(): парсимо один раз
foreground_rgb: rule.foreground_color.as_deref().and_then(parse_hex_color),
background_rgb: rule.background_color.as_deref().and_then(parse_hex_color),
```
В overlay (`highlight_overlay.rs`) `parse_hex_color(bg)` зникає — беремо `m.background_rgb`
напряму. Бонус: `HighlightMatch` стає `Copy` (без `String`), `find_matches` більше
не алокує на збіг.
**Зусилля:** M. **Тести:** оновити наявні юніт-тести `find_matches`.

### 1.2. Дрібна оптимізація `chars().count()` у overlay

**Файл:** `rustconn/src/terminal/highlight_overlay.rs:175-176`
**Проблема:** подвійний O(n) прохід `line[..m.start].chars().count()` і
`line[..m.end].chars().count()` від початку рядка на кожен збіг.
**Рішення:** рахувати `col_end` як дельту: `col_end = col_start + line[m.start..m.end].chars().count()`.
**Зусилля:** S. Робити разом з 1.1.

### 1.3. `sort_group`: `to_lowercase()` у компараторі

**Файл:** `rustconn-core/src/connection/manager.rs:1225`
**Проблема:** `sort_by` з `.to_lowercase()` у компараторі → O(n log n) алокацій.
Поруч (`sort_ids_by_name:1372`) уже є правильний варіант через `sort_by_cached_key`.
**Рішення:** перевикористати наявний патерн `sort_by_cached_key(|x| x.name.to_lowercase())`.
**Зусилля:** S.

### 1.4. Звузити `tokio` features для GUI/CLI

**Файл:** `Cargo.toml:18` (`tokio = { features = ["full"] }`)
**Проблема:** `full` тягнеться у всі крейти, зокрема `rustconn` та `rustconn-cli`,
яким повний набір не потрібен → зайвий час компіляції.
**Рішення:** лишити `full` у workspace-deps лише там, де реально треба; для
GUI/CLI задати точковий набір (напр. `["rt-multi-thread", "macros", "net", "time",
"process", "io-util", "sync"]`). **Обов'язково перевірити збіркою** — звуження
features ризиковане.
**Зусилля:** M (потрібна перевірка, які features фактично використовуються).

---

## P2 — Якість коду та відповідність правилам

### 2.1. Системний перехід `#[allow]` → `#[expect]` (M-LINT-OVERRIDE-EXPECT)

**Файли (приклади):** `rdp_vnc.rs:30,951`, `credentials.rs:535,642`,
`connection_dialogs.rs:220`, `embedded_vnc_types.rs:355`, `rdpdr.rs:961,967,972`,
плюс усі `dead_code`-поля діалогів.
**Проблема:** прагматичні правила вимагають `#[expect(..., reason = "...")]` замість
`#[allow]` — `#[expect]` попереджає, якщо лінт перестав спрацьовувати (не накопичує
застарілі override-и).
```rust
// було
#[allow(dead_code)]
// стало
#[expect(dead_code, reason = "поле резервне для майбутньої RDPDR-функції")]
```
**Додатково:** додати у `[workspace.lints.clippy]` (перевірити, що не ламає збірку):
```toml
allow_attributes_without_reason = "warn"  # змушує reason = "..." в #[allow]/#[expect]
clone_on_ref_ptr = "warn"                 # ловить .clone() на Rc/Arc
```
**Зусилля:** M (механічна заміна по всьому дереву + перевірка clippy).

### 2.2. `rdpdr.rs`: мертве обчислення notify-відповіді

**Файл:** `rustconn-core/src/rdp_client/rdpdr.rs:182-183`
**Проблема:** `build_file_notify_info(&change)` обчислюється, але результат **не
надсилається** — ironrdp ще не має `ClientDriveNotifyChangeDirectoryResponse`. Це
мертвий код + неповна RDPDR-функція.
**Рішення:** прибрати обчислення (залишивши TODO з форматом MS-RDPEFS у коментарі)
АБО загейтити за `#[cfg]`/feature, щоб не виконувати марну роботу. Перевірити, чи
ironrdp 0.16 уже додав тип відповіді.
**Зусилля:** S.

### 2.3. Прибрати дормантний `TabSplitManager` + відновлення split у Workspaces

**Контекст — два механізми, що перекриваються.** Після перевірки коду з'ясовано,
що базовий split-view **працює** і активно використовується:

- **Активний (per-session):** `SessionSplitBridges = HashMap<Uuid, Rc<SplitViewBridge>>`
  (`window/types.rs:196`). Дії `win.split-horizontal`/`win.split-vertical`
  (`window/split_view_actions.rs`), кнопки хедер-бару (`window/ui.rs:117-128`),
  шорткати Ctrl+Shift+S/H. `SplitViewBridge` усередині використовує
  `SplitViewAdapter` → `SplitLayoutModel` (нове tree-ядро з
  `rustconn-core/src/split/`). Кольори, broadcast, eviction — працюють.
- **Дормантний (per-tab):** `TabSplitManager` (`terminal/mod.rs:125`) — амбітніший
  layout, прив'язаний до вкладки (`TabId`). Поле створюється (248) і очищується
  при закритті вкладки (343), але **вся поверхня керування — мертвий код**:
  `split_manager():2701`, `tab_containers():2384`, `get_or_create_tab_id():2719`,
  `get_tab_id():2736`, `is_session_split():2750`, `set_tab_split_color_id():2012`,
  `update_tab_color_indicator():2033`, `get_session_split_color():2769`. Жодного
  виклику з UI до них немає.

**Рішення для 0.17 — Варіант A (deletion over addition):**

1. **Видалити** `TabSplitManager` та пов'язані мертві методи з `terminal/mod.rs`
   (зняти всі `#[allow(dead_code, reason = "...window integration...")]`), залишивши
   працюючий `SplitViewBridge`. Прибрати поле `split_manager` і його очищення в
   close-handler, прибрати `session_tab_ids`, якщо воно більше нікуди не веде.
   Перевірити, що `rustconn-core::split::tab_groups::TabGroupManager` / `TabId` після
   цього не лишаються мертвими в ядрі — за потреби видалити і їх.
2. **Реалізувати відновлення split-layout у Workspaces** — конкретна цінність для
   користувача. Зараз `split_layout` зберігається у профілі, але при «Open» НЕ
   відновлюється (`window/workspaces.rs:43`, ponytail).

```rust
// window/workspaces.rs — у set_on_open, після реконекту записів:
// було: лише reconnect entries.
// стало: відновити збережений layout.
if let Some(layout) = ws.split_layout.clone() {
    // apply_layout відтворює дерево панелей через активний SplitViewBridge
    crate::split_view::apply_layout(&notebook_for_open, &layout);
}
```
(Якщо `apply_layout` ще не існує як публічний хелпер — додати тонку обгортку над
наявним `SplitViewBridge`, що приймає збережену модель layout і відтворює сплити.)

**Чому A, а не «завершити per-tab редизайн»:** повноцінний per-tab `TabSplitManager`
дублює вже працюючий per-session механізм, тягне на L + окремий спек і чіпає гарячий
шлях вкладок/терміналу. YAGNI: користувач уже має робочий split; реальний пробіл —
лише відновлення layout у Workspaces. Якщо потреба в per-tab layout підтвердиться
згодом — повернути окремою фічею.

**Зусилля:** M (видалення мертвого коду + `apply_layout` для Workspaces).
**Пов'язане:** перетинається з 2.1 — частина `#[allow(dead_code)]` зникне разом із
видаленням `TabSplitManager`, тож робити 2.3 перед фінальним свіпом 2.1.

### 2.4. `securecrt.rs`: крихкий `unreachable!()`

**Файл:** `rustconn-core/src/import/securecrt.rs:333`
**Уточнення після перевірки:** це **НЕ** паніка на недовіреному вводі — гілка
`Rlogin | Raw` вже повертає `Ok(None)` на рядку 218 до досягнення цього match.
Тобто реальний ризик відсутній. Проблема лише стилістична: два `match` мають
лишатися синхронними, інакше майбутня правка може зробити гілку досяжною.
**Рішення (дешеве загартування):** замінити `unreachable!()` на безпечне
`return Ok(None)` (узгоджено з раннім фільтром), щоб виключити будь-який шанс паніки.
**Зусилля:** XS. Низький пріоритет.

### 2.5. Видалити legacy XOR-шлях розшифрування

**Файл:** `rustconn-core/src/config/settings.rs` (`decrypt_credential` → `xor_cipher_legacy`)
**Проблема:** fallback на XOR для блобів без заголовка `RCSC`. Коментар обіцяв
видалення «у v0.12», але код є на 0.16.13. XOR не дає реального захисту.
**Рішення:** якщо вікно міграції минуло (а воно минуло давно) — видалити XOR-шлях
(deletion over addition). Перед видаленням переконатися, що немає користувачів зі
старим форматом, або лишити одноразову міграцію з логом-попередженням.
**Зусилля:** S. Узгодити з мейнтейнером щодо політики міграції.

### 2.6. Загартування транзитних secret-структур

**Файли:** `secret/keepassxc.rs:45`, `secret/bitwarden.rs:272`, `vnc_client/client.rs:253`
**Проблема:** plain `String` для паролів у короткоживучих serialize-структурах
(запис у KeePassXC/Bitwarden) та неогорнута копія в VNC.
**Рішення:** для keepassxc/bitwarden — `SecretString` або `Zeroizing` для проміжних
копій під час побудови JSON; для VNC — або `Zeroizing`, або додати пояснювальний
коментар, як це зроблено в SPICE (`spice_client/client.rs:446`).
**Зусилля:** S.

---

## P3 — GNOME HIG / доступність (полірування)

### 3.1. `autotype.rs`: сире `gtk4::Window` замість adw-патерну

**Файл:** `rustconn/src/embedded_rdp/autotype.rs:147-151`
**Проблема:** діалог введення тексту на сирому `gtk4::Window::new()` — єдине вікно
поза libadwaita-патерном. На Wayland модальне сире `gtk4::Window` виглядає окремим
вікном (анти-патерн з gnome-hig.md).
**Рішення:** перевести на `adw::Dialog` з `adw::ToolbarView` + `adw::HeaderBar`,
як решта діалогів. **Зусилля:** S.

### 3.2. Дрібне a11y-полірування (об'єднати в один підхід)

- **Tap-target 44×44** для іконкових кнопок тулбарів (`set_size_request(44, 44)`) —
  HIG Pointer & Touch. Зараз не задано.
- **Мінімальний розмір вікна** 360×294 не гарантовано (`window/mod.rs:171` лише
  відновлює геометрію). Розглянути явний `set_size_request` або довіритися
  `AdwBreakpoint`.
**Оцінка:** низький пріоритет для desktop-first застосунку; зробити лише якщо буде
час. **Зусилля:** S.

---

## Свідомо відкинуто (критична оцінка)

| Знахідка | Чому НЕ йде в 0.17 |
|----------|--------------------|
| `mimalloc` як глобальний алокатор | Спекулятивно без профілю. Правила забороняють спекулятивні оптимізації; додавати лише якщо профіль покаже, що алокація — bottleneck. |
| Дедуплікація hardcoded margins (`12`/`24`px) | YAGNI. Значення правильні за HIG, ризик дрейфу мінімальний, виграш косметичний. |
| `println!`/`eprintln!` у `main.rs` | Легітимні — викликаються **до** ініціалізації `tracing`, у бінарному GUI-крейті. Не порушення. |
| Помилка історії як `AlertDialog` (`dialogs/history.rs:334`) | Прийнятно за HIG (модальна помилка з дією). Заміна на банер — смакове. |
| «`securecrt` panic на untrusted input» | **Спростовано перевіркою**: гілка захищена раннім `return Ok(None)`. Лишилося лише дрібне стилістичне загартування (див. 2.4). |
| `sort_all` O(G²+G·C) (`manager.rs:1296`) | Пом'якшено прапором `is_sorted`, викликається рідко. Достатньо додати `// ponytail:` коментар, окремий фікс не потрібен. |

---

## Чек-лист релізу 0.17.0

- [ ] P0.1 kubectl argv/екранування + Zero-Trust Generic поза Flatpak
- [ ] P0.2 рішення щодо ironrdp `catch_unwind` (TODO(0.17))
- [ ] P0.3 Passbolt — Known Issue в `SECURITY.md` + перевірка upstream stdin
- [ ] P0.4 документування моделі загроз machine-key
- [ ] P1.1 пре-парсинг кольорів підсвічування (+ оновити тести)
- [ ] P1.2 дельта `chars().count()` в overlay
- [ ] P1.3 `sort_group` → `sort_by_cached_key`
- [ ] P1.4 звуження `tokio` features (з перевіркою збірки)
- [ ] P2.1 `#[allow]`→`#[expect]` sweep + restriction-лінти
- [ ] P2.2 `rdpdr` мертве обчислення notify
- [ ] P2.3 видалити дормантний `TabSplitManager` + `apply_layout` для Workspaces (Варіант A)
- [ ] P2.4 `securecrt` `unreachable!()` → `Ok(None)`
- [ ] P2.5 видалити legacy XOR-шлях
- [ ] P2.6 `Zeroizing`/`SecretString` у keepassxc/bitwarden/vnc
- [ ] P3.1 `autotype` → `adw::Dialog`
- [ ] P3.2 a11y-полірування (за наявності часу)
- [ ] `cargo fmt --all` + `cargo clippy --all-targets` (0 warnings)
- [ ] `cargo test --workspace` зелений
- [ ] нові i18n-рядки → `bash po/update-pot.sh` + `msgmerge` для 16 мов
- [ ] оновити версію `0.16.13` → `0.17.0` у `Cargo.toml`
- [ ] оновити `CHANGELOG.md`
