//! Terminal configuration
//!
//! This module handles VTE terminal appearance and behavior configuration.

use gtk4::gdk;
use gtk4::glib;
use gtk4::prelude::*;
use rustconn_core::config::TerminalSettings;
use rustconn_core::models::ConnectionThemeOverride;
use rustconn_core::terminal_themes::{Color, TerminalTheme};
use vte4::prelude::*;
use vte4::{CursorBlinkMode, CursorShape, Terminal};

/// Configures terminal with specific settings
pub fn configure_terminal_with_settings(terminal: &Terminal, settings: &TerminalSettings) {
    // Cursor settings
    let cursor_blink = match settings.cursor_blink.as_str() {
        "On" => CursorBlinkMode::On,
        "Off" => CursorBlinkMode::Off,
        "System" => CursorBlinkMode::System,
        _ => CursorBlinkMode::On,
    };
    terminal.set_cursor_blink_mode(cursor_blink);

    let cursor_shape = match settings.cursor_shape.as_str() {
        "Block" => CursorShape::Block,
        "IBeam" => CursorShape::Ibeam,
        "Underline" => CursorShape::Underline,
        _ => CursorShape::Block,
    };
    terminal.set_cursor_shape(cursor_shape);

    // Scrolling behavior
    terminal.set_scroll_on_output(settings.scroll_on_output);
    terminal.set_scroll_on_keystroke(settings.scroll_on_keystroke);
    terminal.set_scrollback_lines(i64::from(settings.scrollback_lines));

    // Input handling
    terminal.set_input_enabled(true);
    terminal.set_allow_hyperlink(settings.allow_hyperlinks);
    terminal.set_mouse_autohide(settings.mouse_autohide);

    // Bell
    terminal.set_audible_bell(settings.audible_bell);

    // Copy on select (X11-style auto-copy)
    if settings.copy_on_select {
        setup_copy_on_select(terminal);
    }

    // Keyboard shortcuts (Copy/Paste + font zoom)
    setup_keyboard_shortcuts(terminal);
    setup_font_zoom(terminal);

    // Context menu (Right click) — attached to the container, NOT the
    // terminal, to avoid interfering with VTE's internal mouse handling.
    // The container is set up separately after the terminal is placed
    // in the widget tree (see `setup_context_menu_on_container`).

    // Colors and font
    setup_colors_with_theme(terminal, &settings.color_theme);
    setup_font_with_settings(terminal, settings);
}

/// Automatically copies selected text to the clipboard when the user
/// finishes a selection (X11-style "copy on select").
fn setup_copy_on_select(terminal: &Terminal) {
    let term = terminal.clone();
    terminal.connect_selection_changed(move |_| {
        if term.has_selection() {
            term.copy_clipboard_format(vte4::Format::Text);
        }
    });
}

/// Sets up keyboard shortcuts for copy/paste
fn setup_keyboard_shortcuts(terminal: &Terminal) {
    let controller = gtk4::EventControllerKey::new();
    let term = terminal.clone();
    controller.connect_key_pressed(move |_, key, _, state| {
        let mask = gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::SHIFT_MASK;
        if state.contains(mask) {
            match key.name().as_deref() {
                Some("C" | "c") => {
                    term.copy_clipboard_format(vte4::Format::Text);
                    return glib::Propagation::Stop;
                }
                Some("V" | "v") => {
                    term.paste_clipboard();
                    return glib::Propagation::Stop;
                }
                _ => (),
            }
        }
        glib::Propagation::Proceed
    });
    terminal.add_controller(controller);
}

/// Sets up context menu using VTE's native `set_context_menu_model` API.
///
/// VTE 0.76+ handles right-click internally: it preserves the text
/// selection, positions the popover correctly, and routes actions through
/// the widget hierarchy. The previous approach (a `GestureClick` on the
/// container) broke Copy/Paste because the popover stole focus from VTE
/// before the action callbacks could run (#84).
///
/// Copy uses `text_selected()` to snapshot the selection text before VTE
/// clears it, avoiding the `gdk_clipboard_write_async: mime_type != NULL`
/// assertion that `copy_clipboard_format` triggers on an empty selection.
pub fn setup_context_menu_on_container(container: &impl IsA<gtk4::Widget>, terminal: &Terminal) {
    use gtk4::gio;
    use std::cell::RefCell;
    use std::rc::Rc;

    // Cache the last selection so Copy still works after VTE clears it
    // on right-click.
    let last_selection: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));

    // Snapshot selection whenever it changes (including when it appears
    // and when VTE clears it on right-click).
    let sel = last_selection.clone();
    let term_sel = terminal.clone();
    terminal.connect_selection_changed(move |_| {
        if term_sel.has_selection() {
            *sel.borrow_mut() = term_sel
                .text_selected(vte4::Format::Text)
                .map(|s| s.to_string());
        }
        // When selection is cleared we intentionally keep the cached
        // value so the Copy action can still use it.
    });

    // Build the menu model (shared across all right-click invocations)
    let menu = gio::Menu::new();
    let clipboard_section = gio::Menu::new();
    clipboard_section.append(Some(&crate::i18n::i18n("Copy")), Some("terminal.copy"));
    clipboard_section.append(Some(&crate::i18n::i18n("Paste")), Some("terminal.paste"));
    clipboard_section.append(
        Some(&crate::i18n::i18n("Select All")),
        Some("terminal.select-all"),
    );
    menu.append_section(None, &clipboard_section);

    // Register the action group on the container so VTE's popover can
    // resolve "terminal.*" actions by walking up the widget tree.
    let action_group = gio::SimpleActionGroup::new();

    let term_copy = terminal.clone();
    let sel_copy = last_selection;
    let action_copy = gio::SimpleAction::new("copy", None);
    action_copy.connect_activate(move |_, _| {
        // Prefer live selection; fall back to cached snapshot.
        if term_copy.has_selection() {
            term_copy.copy_clipboard_format(vte4::Format::Text);
        } else if let Some(ref text) = *sel_copy.borrow() {
            let display = term_copy.display();
            display.clipboard().set_text(text);
        }
    });
    action_group.add_action(&action_copy);

    let term_paste = terminal.clone();
    let action_paste = gio::SimpleAction::new("paste", None);
    action_paste.connect_activate(move |_, _| {
        term_paste.paste_clipboard();
    });
    action_group.add_action(&action_paste);

    let term_select = terminal.clone();
    let action_select = gio::SimpleAction::new("select-all", None);
    action_select.connect_activate(move |_, _| {
        term_select.select_all();
    });
    action_group.add_action(&action_select);

    // Install on the container (parent of VTE) so the action group is
    // reachable from VTE's internally-created popover.
    container.insert_action_group("terminal", Some(&action_group));

    // Let VTE handle the right-click popover natively.
    terminal.set_context_menu_model(Some(&menu));
}

/// Converts Color to gdk::RGBA
fn color_to_rgba(color: &Color) -> gdk::RGBA {
    gdk::RGBA::new(color.r, color.g, color.b, 1.0)
}

/// Sets up terminal colors with theme
fn setup_colors_with_theme(terminal: &Terminal, theme_name: &str) {
    let theme = TerminalTheme::by_name(theme_name).unwrap_or_else(TerminalTheme::dark_theme);

    let bg_color = color_to_rgba(&theme.background);
    let fg_color = color_to_rgba(&theme.foreground);
    let cursor_color = color_to_rgba(&theme.cursor);

    terminal.set_color_background(&bg_color);
    terminal.set_color_foreground(&fg_color);
    terminal.set_color_cursor(Some(&cursor_color));

    // Set up palette colors
    let palette_rgba: Vec<gdk::RGBA> = theme.palette.iter().map(color_to_rgba).collect();
    let palette_refs: Vec<&gdk::RGBA> = palette_rgba.iter().collect();
    terminal.set_colors(Some(&fg_color), Some(&bg_color), &palette_refs);
}

/// Minimum font scale factor (roughly 50% of base size)
const FONT_SCALE_MIN: f64 = 0.5;
/// Maximum font scale factor (roughly 400% of base size)
const FONT_SCALE_MAX: f64 = 4.0;
/// Step for each zoom increment/decrement
const FONT_SCALE_STEP: f64 = 0.1;

/// Sets up font zoom via Ctrl+Scroll, Ctrl+Plus/Minus, and Ctrl+0 to reset.
///
/// Uses VTE's built-in `set_font_scale()` which scales the configured font
/// without changing the underlying `FontDescription`. This means the zoom
/// level is per-terminal and resets when a new session is created.
fn setup_font_zoom(terminal: &Terminal) {
    // Ctrl+Scroll wheel zoom
    let scroll_controller =
        gtk4::EventControllerScroll::new(gtk4::EventControllerScrollFlags::VERTICAL);
    let term_scroll = terminal.clone();
    scroll_controller.connect_scroll(move |_, _, dy| {
        let state = gdk::Display::default()
            .and_then(|d| d.default_seat())
            .and_then(|s| s.keyboard())
            .map(|k| k.modifier_state())
            .unwrap_or_else(gdk::ModifierType::empty);

        if !state.contains(gdk::ModifierType::CONTROL_MASK) {
            return glib::Propagation::Proceed;
        }

        let current = term_scroll.font_scale();
        let new_scale = if dy < 0.0 {
            (current + FONT_SCALE_STEP).min(FONT_SCALE_MAX)
        } else {
            (current - FONT_SCALE_STEP).max(FONT_SCALE_MIN)
        };
        term_scroll.set_font_scale(new_scale);
        glib::Propagation::Stop
    });
    terminal.add_controller(scroll_controller);

    // Ctrl+Plus / Ctrl+Minus / Ctrl+0 keyboard zoom
    let key_controller = gtk4::EventControllerKey::new();
    let term_key = terminal.clone();
    key_controller.connect_key_pressed(move |_, key, _, state| {
        if !state.contains(gdk::ModifierType::CONTROL_MASK) {
            return glib::Propagation::Proceed;
        }

        match key.name().as_deref() {
            Some("plus" | "equal" | "KP_Add") => {
                let s = (term_key.font_scale() + FONT_SCALE_STEP).min(FONT_SCALE_MAX);
                term_key.set_font_scale(s);
                glib::Propagation::Stop
            }
            Some("minus" | "KP_Subtract") => {
                let s = (term_key.font_scale() - FONT_SCALE_STEP).max(FONT_SCALE_MIN);
                term_key.set_font_scale(s);
                glib::Propagation::Stop
            }
            Some("0" | "KP_0") => {
                term_key.set_font_scale(1.0);
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    terminal.add_controller(key_controller);
}

/// Sets up terminal font with settings
fn setup_font_with_settings(terminal: &Terminal, settings: &TerminalSettings) {
    let font_desc = gtk4::pango::FontDescription::from_string(&format!(
        "{} {}",
        settings.font_family, settings.font_size
    ));
    terminal.set_font(Some(&font_desc));
}

/// Converts a hex color string (`#RRGGBB` or `#RRGGBBAA`) to a GDK RGBA value.
///
/// Returns `None` if the string is not a valid hex color.
fn hex_to_rgba(hex: &str) -> Option<gdk::RGBA> {
    let hex = hex.strip_prefix('#')?;
    let (r, g, b, a) = match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 255u8)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };
    Some(gdk::RGBA::new(
        f32::from(r) / 255.0,
        f32::from(g) / 255.0,
        f32::from(b) / 255.0,
        f32::from(a) / 255.0,
    ))
}

/// Applies per-connection theme override colors to a VTE terminal.
///
/// For each color field present in the override, converts the hex string to
/// RGBA and applies it via the corresponding VTE setter. Fields that are
/// `None` are left unchanged (the global theme colors remain).
pub fn apply_theme_override(terminal: &Terminal, theme_override: &ConnectionThemeOverride) {
    if let Some(ref bg) = theme_override.background
        && let Some(rgba) = hex_to_rgba(bg)
    {
        terminal.set_color_background(&rgba);
    }
    if let Some(ref fg) = theme_override.foreground
        && let Some(rgba) = hex_to_rgba(fg)
    {
        terminal.set_color_foreground(&rgba);
    }
    if let Some(ref cursor) = theme_override.cursor
        && let Some(rgba) = hex_to_rgba(cursor)
    {
        terminal.set_color_cursor(Some(&rgba));
    }
}
