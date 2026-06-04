//! Keybindings settings tab
//!
//! Provides a preferences page for viewing and customizing keyboard shortcuts.
//! Each shortcut is displayed in a row grouped by category, with the ability
//! to record a new accelerator or reset to the default.

use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{Button, EventControllerKey, Label, gio};
use libadwaita as adw;
use rustconn_core::config::keybindings::{
    KeybindingCategory, KeybindingSettings, default_keybindings, is_valid_accelerator,
};
use std::cell::RefCell;
use std::rc::Rc;

use crate::i18n::{i18n, i18n_f};

/// Creates the keybindings preferences page.
///
/// Returns `(page, overrides_cell)` where `overrides_cell` holds the current
/// user overrides and is updated live as the user records new shortcuts.
///
/// Each category is rendered as a collapsible `ExpanderRow` inside a single
/// `PreferencesGroup`, keeping the Interface page compact.
#[expect(
    clippy::too_many_lines,
    reason = "long match/dispatch over many enum variants; splitting per variant only relocates the boilerplate"
)]
pub fn create_keybindings_page() -> (adw::PreferencesPage, Rc<RefCell<KeybindingSettings>>) {
    let page = adw::PreferencesPage::builder()
        .title(&i18n("Keybindings"))
        .icon_name("preferences-desktop-keyboard-symbolic")
        .build();

    let overrides_cell: Rc<RefCell<KeybindingSettings>> =
        Rc::new(RefCell::new(KeybindingSettings::default()));

    let defaults = default_keybindings();

    // Single group for all keybinding categories (collapsible expanders)
    let group = adw::PreferencesGroup::builder()
        .title(&i18n("Keyboard Shortcuts"))
        .build();

    // Build one ExpanderRow per category
    for category in KeybindingCategory::all() {
        let cat_defs: Vec<_> = defaults
            .iter()
            .filter(|d| d.category == *category)
            .collect();
        if cat_defs.is_empty() {
            continue;
        }

        let expander = adw::ExpanderRow::builder()
            .title(&i18n(category.label()))
            .show_enable_switch(false)
            .build();

        for def in &cat_defs {
            let row = adw::ActionRow::builder()
                .title(&i18n(&def.label))
                .subtitle(&def.action)
                .build();

            // Current accelerator label
            let accel_label = Label::builder()
                .label(&def.default_accels)
                .css_classes(["dim-label"])
                .valign(gtk4::Align::Center)
                .build();

            // Record button
            let record_btn = Button::builder()
                .label(&i18n("Record"))
                .valign(gtk4::Align::Center)
                .tooltip_text(&i18n("Press a key combination to set a new shortcut"))
                .build();

            // Reset button
            let reset_btn = Button::builder()
                .icon_name("edit-undo-symbolic")
                .valign(gtk4::Align::Center)
                .tooltip_text(&i18n("Reset to default"))
                .css_classes(["flat"])
                .build();
            reset_btn.update_property(&[gtk4::accessible::Property::Label(&i18n(
                "Reset keybinding to default",
            ))]);

            row.add_suffix(&accel_label);
            row.add_suffix(&record_btn);
            row.add_suffix(&reset_btn);

            // --- Record button handler ---
            let action_name = def.action.clone();
            let default_accels = def.default_accels.clone();
            let accel_label_clone = accel_label.clone();
            let overrides_clone = overrides_cell.clone();
            let record_btn_clone = record_btn.clone();

            record_btn.connect_clicked(move |btn| {
                // Switch to "recording" mode
                btn.set_label(&i18n("Press keys..."));
                btn.set_sensitive(false);

                // Temporarily remove all application accelerators so they do not
                // intercept key events before our EventControllerKey receives them.
                // See: https://github.com/totoshko88/RustConn/issues/167
                let app = gio::Application::default()
                    .and_then(|a| a.downcast::<gtk4::Application>().ok());
                if let Some(ref app) = app {
                    suspend_accels(app);
                }

                // Disable PreferencesDialog search during recording.
                // When search_enabled=true, libadwaita sets a key_capture_widget
                // on the SearchEntry which intercepts letter keys in bubble phase,
                // interfering with shortcut recording.
                let prefs_dialog = btn
                    .ancestor(adw::PreferencesDialog::static_type())
                    .and_then(|w| w.downcast::<adw::PreferencesDialog>().ok());
                if let Some(ref dlg) = prefs_dialog {
                    dlg.set_search_enabled(false);
                }

                // Create a temporary key controller in Capture phase so it
                // receives key events before any child widget or mnemonic.
                let key_ctrl = EventControllerKey::new();
                key_ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);

                let action = action_name.clone();
                let defaults_str = default_accels.clone();
                let label = accel_label_clone.clone();
                let overrides = overrides_clone.clone();
                let record = record_btn_clone.clone();
                let app_for_restore = app.clone();
                let prefs_dialog_restore = prefs_dialog.clone();

                key_ctrl.connect_key_pressed(move |ctrl, keyval, _keycode, modifier| {
                    // Ignore lone modifier presses
                    if is_modifier_key(keyval) {
                        return gtk4::glib::Propagation::Proceed;
                    }

                    // Escape cancels recording
                    if keyval == gtk4::gdk::Key::Escape {
                        label.set_label(
                            &overrides
                                .borrow()
                                .overrides
                                .get(&action)
                                .cloned()
                                .unwrap_or_else(|| defaults_str.clone()),
                        );
                        record.set_label(&i18n("Record"));
                        record.set_sensitive(true);
                        if let Some(ref app) = app_for_restore {
                            restore_accels_with_overrides(app, &overrides.borrow());
                        }
                        if let Some(ref dlg) = prefs_dialog_restore {
                            dlg.set_search_enabled(true);
                        }
                        if let Some(widget) = ctrl.widget() {
                            widget.remove_controller(ctrl);
                        }
                        return gtk4::glib::Propagation::Stop;
                    }

                    // Build accelerator string
                    let accel = gtk4::accelerator_name(keyval, modifier);
                    if is_valid_accelerator(&accel) {
                        // Check for conflicts with other actions
                        let conflict = find_accel_conflict(&accel, &action, &overrides.borrow());
                        if let Some(conflict_label) = &conflict {
                            // Show conflict warning but still allow the assignment
                            let warning = i18n_f("Conflicts with: {}", &[conflict_label]);
                            label.set_label(&format!("{accel}  \u{26A0}"));
                            label.set_tooltip_text(Some(&warning));
                            label.remove_css_class("dim-label");
                            label.add_css_class("warning");
                        } else {
                            label.set_label(&accel);
                            label.set_tooltip_text(None);
                            label.remove_css_class("warning");
                            label.add_css_class("dim-label");
                        }
                        overrides
                            .borrow_mut()
                            .overrides
                            .insert(action.clone(), accel.to_string());
                    }

                    record.set_label(&i18n("Record"));
                    record.set_sensitive(true);
                    if let Some(ref app) = app_for_restore {
                        restore_accels_with_overrides(app, &overrides.borrow());
                    }
                    if let Some(ref dlg) = prefs_dialog_restore {
                        dlg.set_search_enabled(true);
                    }
                    if let Some(widget) = ctrl.widget() {
                        widget.remove_controller(ctrl);
                    }
                    gtk4::glib::Propagation::Stop
                });

                // Attach key controller to the toplevel window and ensure it
                // has focus so key events are delivered during recording.
                // When set_sensitive(false) is called above, the button loses
                // focus and without an explicit focus target GTK4 may not
                // deliver key events (no target widget for propagation).
                if let Some(toplevel) = btn.root() {
                    toplevel.add_controller(key_ctrl);
                    // Move focus to the parent row; this ensures GTK4 has a
                    // valid target widget for key event propagation.
                    if let Some(row) = btn.ancestor(adw::ActionRow::static_type()) {
                        row.grab_focus();
                    } else if let Some(window) = toplevel.downcast_ref::<gtk4::Window>() {
                        // Fallback: clear focus to let Window itself be target
                        gtk4::prelude::GtkWindowExt::set_focus(window, gtk4::Widget::NONE);
                    }
                }
            });

            // --- Reset button handler ---
            let action_name = def.action.clone();
            let default_accels = def.default_accels.clone();
            let overrides_clone = overrides_cell.clone();

            reset_btn.connect_clicked(move |_| {
                overrides_clone.borrow_mut().reset(&action_name);
                accel_label.set_label(&default_accels);
            });

            expander.add_row(&row);
        }

        group.add(&expander);
    }

    page.add(&group);

    // Reset All button at the bottom
    let reset_all_group = adw::PreferencesGroup::new();
    let reset_all_btn = Button::builder()
        .label(&i18n("Reset All to Defaults"))
        .css_classes(["destructive-action"])
        .halign(gtk4::Align::Center)
        .build();

    let overrides_clone = overrides_cell.clone();
    let page_clone = page.clone();
    reset_all_btn.connect_clicked(move |_| {
        overrides_clone.borrow_mut().reset_all();
        // Refresh all labels by removing and re-adding the page content
        // Simpler: just update all dim-label Labels
        refresh_accel_labels(&page_clone);
    });

    reset_all_group.add(&reset_all_btn);
    page.add(&reset_all_group);

    (page, overrides_cell)
}

/// Loads keybinding settings into the page by updating accelerator labels.
pub fn load_keybinding_settings(
    page: &adw::PreferencesPage,
    overrides_cell: &Rc<RefCell<KeybindingSettings>>,
    settings: &KeybindingSettings,
) {
    *overrides_cell.borrow_mut() = settings.clone();

    // Collect all ActionRow widgets recursively and update their labels
    let defaults = default_keybindings();
    let mut action_rows: Vec<gtk4::Widget> = Vec::new();
    collect_action_rows(&page.clone().upcast::<gtk4::Widget>(), &mut action_rows);

    for (row_widget, def) in action_rows.iter().zip(defaults.iter()) {
        let accel = settings.get_accel(def);
        update_row_accel_label(row_widget, accel);
    }
}

/// Collects the current keybinding overrides from the page state.
pub fn collect_keybinding_settings(
    overrides_cell: &Rc<RefCell<KeybindingSettings>>,
) -> KeybindingSettings {
    overrides_cell.borrow().clone()
}

/// Checks whether `accel` conflicts with another action's shortcut.
///
/// Returns the human-readable label of the conflicting action, or `None`.
fn find_accel_conflict(
    accel: &str,
    current_action: &str,
    overrides: &KeybindingSettings,
) -> Option<String> {
    let defaults = default_keybindings();
    for def in &defaults {
        if def.action == current_action {
            continue;
        }
        let effective = overrides.get_accel(def);
        // Check each pipe-separated accelerator
        for existing in effective.split('|') {
            if existing == accel {
                return Some(def.label.clone());
            }
        }
    }
    None
}

/// Returns `true` if the keyval is a modifier key (Shift, Control, Alt, Super).
fn is_modifier_key(keyval: gtk4::gdk::Key) -> bool {
    matches!(
        keyval,
        gtk4::gdk::Key::Shift_L
            | gtk4::gdk::Key::Shift_R
            | gtk4::gdk::Key::Control_L
            | gtk4::gdk::Key::Control_R
            | gtk4::gdk::Key::Alt_L
            | gtk4::gdk::Key::Alt_R
            | gtk4::gdk::Key::Super_L
            | gtk4::gdk::Key::Super_R
            | gtk4::gdk::Key::Meta_L
            | gtk4::gdk::Key::Meta_R
            | gtk4::gdk::Key::Hyper_L
            | gtk4::gdk::Key::Hyper_R
            | gtk4::gdk::Key::ISO_Level3_Shift
    )
}

/// Refreshes all accelerator labels in the page to show defaults.
fn refresh_accel_labels(page: &adw::PreferencesPage) {
    let defaults = default_keybindings();
    let mut action_rows: Vec<gtk4::Widget> = Vec::new();
    collect_action_rows(&page.clone().upcast::<gtk4::Widget>(), &mut action_rows);

    for (row_widget, def) in action_rows.iter().zip(defaults.iter()) {
        update_row_accel_label(row_widget, &def.default_accels);
    }
}

/// Recursively collects all `ActionRow` widgets from a widget tree.
///
/// Skips `ExpanderRow` itself (which is also an `ActionRow` subclass) and
/// only collects leaf `ActionRow` widgets that represent keybinding entries.
fn collect_action_rows(widget: &gtk4::Widget, rows: &mut Vec<gtk4::Widget>) {
    // ExpanderRow is a subclass of PreferencesRow, not ActionRow, so
    // checking `is::<adw::ActionRow>()` won't match it. But to be safe,
    // skip any ExpanderRow explicitly.
    if widget.is::<adw::ExpanderRow>() {
        // Still recurse into its children to find nested ActionRows
        let mut child = widget.first_child();
        while let Some(w) = child {
            collect_action_rows(&w, rows);
            child = w.next_sibling();
        }
        return;
    }

    if widget.is::<adw::ActionRow>() {
        rows.push(widget.clone());
        return;
    }

    let mut child = widget.first_child();
    while let Some(w) = child {
        collect_action_rows(&w, rows);
        child = w.next_sibling();
    }
}

/// Finds and updates the accelerator label within an `ActionRow`.
fn update_row_accel_label(row_widget: &gtk4::Widget, accel: &str) {
    // The suffix box is the last child of the ActionRow's internal layout.
    // Walk children looking for a Label with the "dim-label" CSS class.
    let mut child = row_widget.first_child();
    while let Some(w) = child {
        if let Some(label) = w.downcast_ref::<Label>()
            && label.has_css_class("dim-label")
        {
            label.set_label(accel);
            return;
        }
        // Check nested children (suffix box)
        let mut inner = w.first_child();
        while let Some(inner_w) = inner {
            if let Some(label) = inner_w.downcast_ref::<Label>()
                && label.has_css_class("dim-label")
            {
                label.set_label(accel);
                return;
            }
            // One more level deep for the suffix box
            let mut deep = inner_w.first_child();
            while let Some(deep_w) = deep {
                if let Some(label) = deep_w.downcast_ref::<Label>()
                    && label.has_css_class("dim-label")
                {
                    label.set_label(accel);
                    return;
                }
                deep = deep_w.next_sibling();
            }
            inner = inner_w.next_sibling();
        }
        child = w.next_sibling();
    }
}

/// Temporarily removes all application accelerators.
///
/// This prevents global shortcuts (e.g. `Ctrl+W` for close-tab) from
/// intercepting key events while the user is recording a new shortcut.
/// Call [`restore_accels_with_overrides`] after recording completes or is cancelled.
///
/// See: <https://github.com/totoshko88/RustConn/issues/167>
fn suspend_accels(app: &gtk4::Application) {
    let defaults = default_keybindings();
    for def in &defaults {
        app.set_accels_for_action(&def.action, &[]);
    }
}

/// Restores all application accelerators respecting user overrides.
///
/// This re-applies the currently effective accelerators (user overrides
/// where present, defaults otherwise) after a recording session has ended.
/// Also called on dialog close to guarantee accelerators are never left empty.
pub fn restore_accels_with_overrides(app: &gtk4::Application, overrides: &KeybindingSettings) {
    let defaults = default_keybindings();
    for def in &defaults {
        let effective = overrides.get_accel(def);
        let accels: Vec<&str> = effective.split('|').collect();
        app.set_accels_for_action(&def.action, &accels);
    }
}
