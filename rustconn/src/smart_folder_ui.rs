//! Smart Folders sidebar section widget.
//!
//! Provides a collapsible "Smart Folders" section for the sidebar with:
//! - A header with 🔍 icon and "Add" button
//! - A `ListBox` showing each smart folder with name and match count
//! - Context menu with Edit / Delete actions
//! - Read-only view (no drag-drop)

use crate::i18n::i18n;
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Label, ListBox, Orientation, SelectionMode, Widget};
use rustconn_core::models::{Connection, SmartFolder};
use rustconn_core::smart_folder::SmartFolderManager;

/// Sidebar section that displays smart folders with dynamic connection counts.
pub struct SmartFoldersSidebar {
    /// Root container widget.
    container: GtkBox,
    /// The list box holding smart folder rows.
    list_box: ListBox,
    /// Header label (kept alive for updates).
    #[allow(dead_code)]
    header_label: Label,
    /// Add button (kept alive for signal handler).
    add_button: Button,
}

impl SmartFoldersSidebar {
    /// Creates a new Smart Folders sidebar section.
    #[must_use]
    pub fn new() -> Self {
        let container = GtkBox::new(Orientation::Vertical, 4);
        container.set_margin_top(8);
        container.set_margin_bottom(4);
        container.set_margin_start(8);
        container.set_margin_end(8);

        // --- Header row: icon + label + spacer + add button ---
        let header_row = GtkBox::new(Orientation::Horizontal, 6);
        header_row.set_margin_bottom(4);

        let icon_label = Label::new(Some("🔍"));
        icon_label.add_css_class("heading");
        header_row.append(&icon_label);

        let header_label = Label::new(Some(&i18n("Smart Folders")));
        header_label.add_css_class("heading");
        header_label.set_hexpand(true);
        header_label.set_halign(gtk4::Align::Start);
        header_row.append(&header_label);

        let add_button = Button::from_icon_name("list-add-symbolic");
        add_button.set_tooltip_text(Some(&i18n("New Smart Folder")));
        add_button.add_css_class("flat");
        add_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
            "Create new smart folder",
        ))]);
        header_row.append(&add_button);

        container.append(&header_row);

        // --- Separator ---
        let sep = gtk4::Separator::new(Orientation::Horizontal);
        container.append(&sep);

        // --- List box (read-only, no drag-drop) ---
        let list_box = ListBox::new();
        list_box.set_selection_mode(SelectionMode::Single);
        list_box.add_css_class("navigation-sidebar");
        list_box.set_activate_on_single_click(true);
        container.append(&list_box);

        Self {
            container,
            list_box,
            header_label,
            add_button,
        }
    }

    /// Returns the root GTK widget for embedding in the sidebar.
    #[must_use]
    pub fn widget(&self) -> &Widget {
        self.container.upcast_ref()
    }

    /// Returns a reference to the "Add" button for connecting signals.
    #[must_use]
    pub fn add_button(&self) -> &Button {
        &self.add_button
    }

    /// Returns a reference to the list box for connecting signals.
    #[must_use]
    pub fn list_box(&self) -> &ListBox {
        &self.list_box
    }

    /// Refreshes the list with current smart folders and connections.
    ///
    /// For each folder the manager evaluates matching connections and
    /// displays the folder name with a count badge.
    pub fn update(&self, folders: &[SmartFolder], connections: &[Connection]) {
        // Remove all existing rows
        while let Some(child) = self.list_box.first_child() {
            self.list_box.remove(&child);
        }

        if folders.is_empty() {
            let placeholder = Label::new(Some(&i18n("No smart folders")));
            placeholder.add_css_class("dim-label");
            placeholder.set_margin_top(8);
            placeholder.set_margin_bottom(8);
            self.list_box.append(&placeholder);
            return;
        }

        let manager = SmartFolderManager::new();

        for folder in folders {
            let count = manager.evaluate(folder, connections).len();
            let row = build_folder_row(folder, count);
            self.list_box.append(&row);
        }
    }
}

impl Default for SmartFoldersSidebar {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds a single smart folder row with name and connection count.
fn build_folder_row(folder: &SmartFolder, count: usize) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 8);
    row.set_margin_top(4);
    row.set_margin_bottom(4);
    row.set_margin_start(4);
    row.set_margin_end(4);

    // Folder icon
    let icon = Label::new(Some("📁"));
    row.append(&icon);

    // Folder name
    let name_label = Label::new(Some(&folder.name));
    name_label.set_hexpand(true);
    name_label.set_halign(gtk4::Align::Start);
    name_label.set_ellipsize(gtk4::pango::EllipsizeMode::End);
    row.append(&name_label);

    // Connection count badge
    let count_label = Label::new(Some(&count.to_string()));
    count_label.add_css_class("dim-label");
    row.append(&count_label);

    // Attach context menu via right-click gesture
    let gesture = gtk4::GestureClick::new();
    gesture.set_button(gdk::BUTTON_SECONDARY);
    let folder_id = folder.id;
    gesture.connect_pressed(move |gesture, _n, x, y| {
        if let Some(widget) = gesture.widget() {
            show_smart_folder_context_menu(&widget, x, y, folder_id);
        }
    });
    row.add_controller(gesture);

    row
}

/// Shows a context menu with Edit / Delete for a smart folder row.
fn show_smart_folder_context_menu(
    widget: &impl IsA<gtk4::Widget>,
    x: f64,
    y: f64,
    _folder_id: uuid::Uuid,
) {
    let Some(root) = widget.root() else { return };
    let Some(window) = root.downcast_ref::<gtk4::ApplicationWindow>() else {
        return;
    };

    let popover = gtk4::Popover::new();

    let menu_box = GtkBox::new(Orientation::Vertical, 0);
    menu_box.set_margin_top(6);
    menu_box.set_margin_bottom(6);
    menu_box.set_margin_start(6);
    menu_box.set_margin_end(6);

    let create_menu_button = |label: &str| -> Button {
        let btn = Button::with_label(label);
        btn.set_has_frame(false);
        btn.add_css_class("flat");
        btn.set_halign(gtk4::Align::Start);
        btn
    };

    let popover_ref = popover.downgrade();
    let window_clone = window.clone();

    // Edit
    let edit_btn = create_menu_button(&i18n("Edit"));
    let win = window_clone.clone();
    let popover_c = popover_ref.clone();
    edit_btn.connect_clicked(move |_| {
        if let Some(p) = popover_c.upgrade() {
            p.popdown();
        }
        if let Some(action) = win.lookup_action("edit-smart-folder") {
            action.activate(None);
        }
    });
    menu_box.append(&edit_btn);

    // Delete
    let delete_btn = create_menu_button(&i18n("Delete"));
    delete_btn.add_css_class("destructive-action");
    let win = window_clone;
    let popover_c = popover_ref;
    delete_btn.connect_clicked(move |_| {
        if let Some(p) = popover_c.upgrade() {
            p.popdown();
        }
        if let Some(action) = win.lookup_action("delete-smart-folder") {
            action.activate(None);
        }
    });
    menu_box.append(&delete_btn);

    popover.set_child(Some(&menu_box));
    popover.set_parent(window);

    let widget_bounds = widget.compute_bounds(window);
    #[allow(clippy::cast_possible_truncation)]
    let (popup_x, popup_y) = if let Some(bounds) = widget_bounds {
        (bounds.x() as i32 + x as i32, bounds.y() as i32 + y as i32)
    } else {
        (x as i32, y as i32)
    };

    popover.set_pointing_to(Some(&gdk::Rectangle::new(popup_x, popup_y, 1, 1)));
    popover.set_autohide(true);
    popover.set_has_arrow(true);

    popover.connect_closed(|p| {
        p.unparent();
    });

    popover.popup();
}
