# ТЗ: Рефакторинг Tab Overview + TabView/SplitView архітектури

## Проблема

`AdwTabOverview` не працює коректно з поточною архітектурою, де `TabView` прихований
(`set_visible(false)`) для SSH сесій, а термінали живуть у окремому split view container.

### Поточна архітектура (проблемна)

```
┌─ terminal_container (GtkBox vertical) ─────────────────┐
│  ┌─ TabBar ──────────────────────────────────────────┐  │
│  │  [Welcome] [Shell 1] [Shell 2] [Shell 3]         │  │
│  └───────────────────────────────────────────────────┘  │
│  ┌─ TabView (HIDDEN — set_visible(false)) ───────────┐  │
│  │  TabPage children мають 0×0 allocation            │  │
│  │  Термінали reparented у split view                │  │
│  └───────────────────────────────────────────────────┘  │
│  ┌─ SplitViewBridge (VISIBLE — показує контент) ─────┐  │
│  │  ┌─ Panel A ──────┐  ┌─ Panel B ──────┐          │  │
│  │  │  VTE Terminal   │  │  VTE Terminal   │          │  │
│  │  └────────────────┘  └────────────────┘          │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

**Чому це ламає TabOverview:**
- `AdwTabOverview` робить snapshot кожного `TabPage.child()`
- Всі children мають 0×0 allocation → Pango assertion `size >= 0` → SIGSEGV
- Навіть з workaround (pinning, placeholders) — розміри preview некоректні,
  клік по табу відкриває порожній контент, блимання при повторному відкритті

### Баги які потрібно виправити

1. **SIGSEGV / Pango assertions** при відкритті TabOverview з split-view табами
2. **Порожній контент** при кліку по табу в overview (термінал не повертається з split view)
3. **Некоректний розмір preview** — визначається активним табом (може бути split-view з меншим allocation)
4. **Зависання** при відкритті overview без жодної сесії (тільки Welcome tab)
5. **Блимання** при повторному відкритті overview (множинні cleanup handlers)

---

## Цільова архітектура

### Принцип: TabView завжди видимий, термінали завжди в TabView

```
┌─ terminal_container (GtkBox vertical) ─────────────────┐
│  ┌─ TabOverview (обгортає все) ──────────────────────┐  │
│  │  ┌─ TabBar ────────────────────────────────────┐  │  │
│  │  │  [Shell 1] [Shell 2] [Shell 3]             │  │  │
│  │  └────────────────────────────────────────────┘  │  │
│  │  ┌─ TabView (ЗАВЖДИ VISIBLE) ─────────────────┐  │  │
│  │  │  ┌─ TabPage (selected) ──────────────────┐ │  │  │
│  │  │  │  ┌─ SplitContainer (per-tab) ───────┐ │ │  │  │
│  │  │  │  │  ┌─ Panel A ──┐ ┌─ Panel B ──┐  │ │ │  │  │
│  │  │  │  │  │ VTE Term   │ │ VTE Term   │  │ │ │  │  │
│  │  │  │  │  └────────────┘ └────────────┘  │ │ │  │  │
│  │  │  │  └──────────────────────────────────┘ │ │  │  │
│  │  │  └───────────────────────────────────────┘ │  │  │
│  │  │  ┌─ TabPage (not selected) ──────────────┐ │  │  │
│  │  │  │  VTE Terminal (single pane)            │ │  │  │
│  │  │  └───────────────────────────────────────┘ │  │  │
│  │  └────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

**Ключова зміна:** кожен `TabPage.child()` містить або:
- Один VTE термінал (звичайний таб)
- `SplitContainer` з кількома панелями (split-view таб)
- Welcome screen (коли немає сесій)

Термінали **ніколи не reparent-яться** з TabView. Split view container живе
всередині TabPage, а не як окремий widget поза TabView.

---

## Задачі рефакторингу

### Фаза 1: Підготовка (не ламає існуючий функціонал)

#### 1.1 Створити `TabPageContainer` wrapper
**Файл:** `rustconn/src/terminal/tab_container.rs` (новий)

```rust
/// Контейнер для вмісту TabPage. Може бути в одному з трьох станів:
/// - Single: один VTE термінал
/// - Split: SplitContainer з кількома панелями
/// - Welcome: welcome screen
pub struct TabPageContainer {
    outer: GtkBox,           // Завжди має ненульовий розмір
    state: ContainerState,
}

enum ContainerState {
    Single { terminal_box: GtkBox },
    Split { split_widget: GtkBox },
    Welcome { status_page: AdwStatusPage },
}
```

- `outer` GtkBox завжди `set_hexpand(true)`, `set_vexpand(true)`
- Гарантує що `TabPage.child()` ніколи не має 0×0 allocation
- Методи: `to_split()`, `to_single()`, `get_terminal()`, `get_split_widget()`

#### 1.2 Додати `TabPageContainer` до `TerminalNotebook`
**Файл:** `rustconn/src/terminal/mod.rs`

- Додати `tab_containers: Rc<RefCell<HashMap<Uuid, TabPageContainer>>>`
- `create_terminal_tab_with_settings()` створює `TabPageContainer::single()`
- `TabPage.child()` = `tab_container.outer` (завжди має розмір)

### Фаза 2: Міграція split view всередину TabPage

#### 2.1 Перемістити split container в TabPage
**Файли:**
- `rustconn/src/window/split_view_actions.rs`
- `rustconn/src/split_view/bridge.rs`
- `rustconn/src/terminal/mod.rs`

Зараз при split:
```
1. Термінал reparent з TabPage → SplitViewBridge panel
2. TabView ховається
3. SplitViewBridge widget показується окремо
```

Після рефакторингу:
```
1. TabPageContainer.to_split() — створює split layout всередині TabPage
2. Термінал переміщується з outer box → split panel (всередині того ж TabPage)
3. TabView залишається видимим
```

**Ключові зміни:**
- `SplitViewBridge` створюється для кожного TabPage, а не як глобальний widget
- `split_container` (глобальний `GtkBox` в `window/mod.rs`) — видалити
- `hide_tab_view_content()` / `show_tab_view_content()` — видалити
- Tab switching (`connect_notify("selected-page")`) — спростити:
  просто `tab_view.set_selected_page()`, GTK сам показує правильний контент

#### 2.2 Оновити tab switching логіку
**Файл:** `rustconn/src/window/mod.rs` (рядки ~1870–1980)

Зараз `connect_notify("selected-page")` має складну логіку:
- Перевіряє чи сесія в split bridge
- Ховає/показує глобальний split container
- Reparent-ить термінали між TabView і split view

Після рефакторингу:
```rust
tab_view.connect_notify_local(Some("selected-page"), move |tab_view, _| {
    let Some(page) = tab_view.selected_page() else { return };
    // Все. GTK сам показує правильний TabPage.child().
    // Split view вже всередині TabPage — нічого reparent-ити не треба.

    // Тільки: оновити activity monitor, focus terminal
    if let Some(session_id) = get_session_for_page(&page) {
        activity.on_tab_switched(session_id);
        if let Some(terminal) = get_focused_terminal(session_id) {
            terminal.grab_focus();
        }
    }
});
```

#### 2.3 Оновити split view actions
**Файл:** `rustconn/src/window/split_view_actions.rs`

Зараз `split-horizontal` / `split-vertical`:
1. Створює `SplitViewBridge` як окремий widget
2. Додає його в `split_container` (глобальний)
3. Ховає TabView

Після:
1. Отримує `TabPageContainer` для поточного табу
2. Викликає `container.to_split(orientation)`
3. Split layout створюється всередині TabPage
4. TabView залишається видимим

### Фаза 3: Прибрати workaround-и

#### 3.1 Спростити `open_tab_overview()`
**Файл:** `rustconn/src/terminal/mod.rs`

```rust
pub fn open_tab_overview(&self) {
    if self.sessions.borrow().is_empty() {
        return;
    }
    self.tab_overview.set_open(true);
    // Все. Ніяких placeholder-ів, pinning, set_visible хаків.
}
```

#### 3.2 Видалити непотрібний код
- `hide_tab_view_content()` / `show_tab_view_content()` — видалити
- `reparent_terminal_to_tab()` — видалити (термінали не reparent-яться)
- `setup_tab_overview_cleanup()` — видалити (не потрібен)
- `split_container` в `MainWindow` — видалити
- Глобальний `SplitViewBridge` — замінити на per-tab bridges

#### 3.3 Welcome tab
- Welcome показується як звичайний `TabPage` з `AdwStatusPage`
- Видаляється коли з'являється перша сесія
- Створюється коли закривається остання сесія
- В TabOverview показується нормально (має реальний контент)

### Фаза 4: Тестування

#### 4.1 Сценарії для перевірки
1. Відкрити 5+ табів → TabOverview → всі мають preview → клік по табу → правильний контент
2. Зробити split view → TabOverview → split таб показує split layout в preview
3. Закрити всі таби → Welcome → TabOverview → показує Welcome
4. Split view + звичайні таби → TabOverview → правильні розміри для всіх
5. Повторне відкриття TabOverview → без блимання, без артефактів
6. Ctrl+Shift+O → відкриває overview → Escape → закриває → стан відновлений
7. Tab Switcher (% в Command Palette) → працює як раніше

---

## Файли які потрібно змінити

| Файл | Зміни |
|------|-------|
| `rustconn/src/terminal/tab_container.rs` | **НОВИЙ** — `TabPageContainer` |
| `rustconn/src/terminal/mod.rs` | Інтеграція `TabPageContainer`, спрощення `open_tab_overview`, видалення `hide/show_tab_view_content`, `reparent_terminal_to_tab` |
| `rustconn/src/window/mod.rs` | Видалення `split_container`, спрощення tab switching, видалення reparenting логіки |
| `rustconn/src/window/split_view_actions.rs` | Split створюється всередині TabPage замість глобального container |
| `rustconn/src/split_view/bridge.rs` | Адаптація для роботи всередині TabPage |
| `rustconn/src/split_view/adapter.rs` | Мінімальні зміни (adapter вже працює з будь-яким parent widget) |

## Оцінка складності

- **Фаза 1:** 2-3 години (новий файл + інтеграція)
- **Фаза 2:** 6-8 годин (основний рефакторинг, найскладніша частина)
- **Фаза 3:** 1-2 години (видалення коду)
- **Фаза 4:** 2-3 години (тестування всіх сценаріїв)

**Загалом: ~12-16 годин роботи**

## Ризики

1. **Split view bridge** тісно зв'язаний з глобальним container — потрібно уважно
   відстежити всі місця де `split_container` використовується
2. **Reconnect flow** (`set_on_reconnect`) reparent-ить термінали — потрібно адаптувати
3. **Session recording** (`start_recording`) працює з terminal widget — не повинно зламатись
4. **Monitoring bar** додається до session container — потрібно перевірити що він
   правильно позиціонується всередині TabPage split layout
5. **Tab close** (`connect_close_page`) очищує split state — потрібно адаптувати

## Поточний стан (workaround)

До рефакторингу використовується pin-based workaround:
- Split-view таби та Welcome тимчасово pin-яться перед відкриттям TabOverview
- Pinned таби рендеряться як маленькі іконки без snapshot
- При закритті overview — unpin назад
- **Обмеження:** клік по табу в overview не показує контент, розміри preview некоректні
