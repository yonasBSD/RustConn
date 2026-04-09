//! UI helper functions for connection sidebar
//!
//! This module contains UI-related helper functions for creating popovers,
//! context menus, and other visual elements used by the sidebar widget.

use crate::i18n::i18n;
use gtk4::gdk;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Button, Orientation, gio};

/// Shows the context menu for a connection item with group awareness
///
/// Uses `PopoverMenu` with `gio::Menu` for native GNOME HIG compliance,
/// keyboard navigation, and screen reader accessibility.
#[allow(clippy::fn_params_excessive_bools)]
pub fn show_context_menu_for_item(
    widget: &impl IsA<gtk4::Widget>,
    x: f64,
    y: f64,
    is_group: bool,
    is_ssh: bool,
    is_connected: bool,
    is_recording: bool,
) {
    let Some(root) = widget.root() else { return };
    let Some(window) = root.downcast_ref::<gtk4::ApplicationWindow>() else {
        return;
    };

    let menu = gio::Menu::new();

    if is_group {
        // Group actions
        let group_section = gio::Menu::new();
        group_section.append(
            Some(&i18n("New Connection in Group")),
            Some("win.new-connection-in-group"),
        );
        group_section.append(Some(&i18n("Connect All")), Some("win.connect-all-in-group"));
        menu.append_section(None, &group_section);

        let edit_section = gio::Menu::new();
        edit_section.append(Some(&i18n("Edit")), Some("win.edit-connection"));
        edit_section.append(Some(&i18n("Rename")), Some("win.rename-item"));
        menu.append_section(None, &edit_section);
    } else {
        // Connection actions section
        let connect_section = gio::Menu::new();
        connect_section.append(Some(&i18n("Connect")), Some("win.connect"));
        connect_section.append(Some(&i18n("Pin / Unpin")), Some("win.toggle-pin"));
        menu.append_section(None, &connect_section);

        // Edit section
        let edit_section = gio::Menu::new();
        edit_section.append(
            Some(&i18n("New Connection")),
            Some("win.new-connection-from-context"),
        );
        edit_section.append(Some(&i18n("Edit")), Some("win.edit-connection"));
        edit_section.append(Some(&i18n("Rename")), Some("win.rename-item"));
        edit_section.append(Some(&i18n("Duplicate")), Some("win.duplicate-connection"));
        edit_section.append(Some(&i18n("Move to Group...")), Some("win.move-to-group"));
        menu.append_section(None, &edit_section);

        // Clipboard section
        let clipboard_section = gio::Menu::new();
        clipboard_section.append(Some(&i18n("Copy Username")), Some("win.copy-username"));
        clipboard_section.append(Some(&i18n("Copy Password")), Some("win.copy-password"));
        menu.append_section(None, &clipboard_section);

        // Tools section
        let tools_section = gio::Menu::new();
        tools_section.append(
            Some(&i18n("Run Snippet...")),
            Some("win.run-snippet-for-connection"),
        );
        tools_section.append(Some(&i18n("Wake On LAN")), Some("win.wake-on-lan"));
        tools_section.append(
            Some(&i18n("Check if Online")),
            Some("win.check-host-online"),
        );
        if is_ssh {
            tools_section.append(Some(&i18n("Open SFTP")), Some("win.open-sftp"));
        }
        menu.append_section(None, &tools_section);

        // Recording section (only for connected sessions)
        if is_connected {
            let recording_section = gio::Menu::new();
            if is_recording {
                recording_section.append(Some(&i18n("Stop Recording")), Some("win.stop-recording"));
            } else {
                recording_section
                    .append(Some(&i18n("Start Recording")), Some("win.start-recording"));
            }
            menu.append_section(None, &recording_section);
        }
    }

    // Delete section (always last, visually separated)
    let delete_section = gio::Menu::new();
    delete_section.append(Some(&i18n("Delete")), Some("win.delete-connection"));
    menu.append_section(None, &delete_section);

    show_popover_menu(widget, window, &menu, x, y);
}

/// Shows the context menu for empty space in the sidebar
pub fn show_empty_space_context_menu(widget: &impl IsA<gtk4::Widget>, x: f64, y: f64) {
    let Some(root) = widget.root() else { return };
    let Some(window) = root.downcast_ref::<gtk4::ApplicationWindow>() else {
        return;
    };

    let menu = gio::Menu::new();

    let create_section = gio::Menu::new();
    create_section.append(Some(&i18n("Quick Connect")), Some("win.quick-connect"));
    create_section.append(Some(&i18n("New Connection")), Some("win.new-connection"));
    create_section.append(Some(&i18n("New Group")), Some("win.new-group"));
    menu.append_section(None, &create_section);

    let io_section = gio::Menu::new();
    io_section.append(Some(&i18n("Import...")), Some("win.import"));
    io_section.append(Some(&i18n("Export...")), Some("win.export"));
    menu.append_section(None, &io_section);

    show_popover_menu(widget, window, &menu, x, y);
}

/// Creates and shows a `PopoverMenu` from a `gio::Menu` at the given coordinates
fn show_popover_menu(
    widget: &impl IsA<gtk4::Widget>,
    _window: &gtk4::ApplicationWindow,
    menu: &gio::Menu,
    x: f64,
    y: f64,
) {
    let popover = gtk4::PopoverMenu::from_model(Some(menu));
    popover.set_parent(widget);

    #[allow(clippy::cast_possible_truncation)]
    let rect = gdk::Rectangle::new(x as i32, y as i32, 1, 1);
    popover.set_pointing_to(Some(&rect));
    popover.set_autohide(true);
    popover.set_has_arrow(false);

    popover.connect_closed(|p| {
        p.unparent();
    });

    popover.popup();
}

/// Returns the appropriate icon name for a protocol string
///
/// For ZeroTrust connections, the protocol string may include provider info
/// in the format "zerotrust:provider" (e.g., "zerotrust:aws", "zerotrust:gcloud").
/// All ZeroTrust connections use the same icon regardless of provider.
///
/// Icons are aligned with `rustconn_core::protocol::icons::get_protocol_icon()`.
#[must_use]
pub fn get_protocol_icon(protocol: &str) -> &'static str {
    rustconn_core::get_protocol_icon_by_name(protocol)
}

/// Creates the bulk actions toolbar for group operations mode
#[must_use]
pub fn create_bulk_actions_bar() -> GtkBox {
    let bar = GtkBox::new(Orientation::Horizontal, 4);
    bar.set_margin_start(8);
    bar.set_margin_end(8);
    bar.set_margin_top(4);
    bar.set_margin_bottom(4);
    bar.add_css_class("bulk-actions-bar");

    let new_group_button = Button::with_label(&i18n("New Group"));
    new_group_button.set_tooltip_text(Some(&i18n("Create a new group")));
    new_group_button.set_action_name(Some("win.new-group"));
    new_group_button.add_css_class("suggested-action");
    new_group_button
        .update_property(&[gtk4::accessible::Property::Label(&i18n("Create new group"))]);
    bar.append(&new_group_button);

    let move_button = Button::with_label(&i18n("Move to Group..."));
    move_button.set_tooltip_text(Some(&i18n("Move selected items to a group")));
    move_button.set_action_name(Some("win.move-selected-to-group"));
    move_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Move selected connections to group",
    ))]);
    bar.append(&move_button);

    let cluster_button = Button::with_label(&i18n("Create Cluster"));
    cluster_button.set_tooltip_text(Some(&i18n("Create a cluster from selected connections")));
    cluster_button.set_action_name(Some("win.cluster-from-selection"));
    cluster_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Create cluster from selected connections",
    ))]);
    bar.append(&cluster_button);

    let select_all_button = Button::with_label(&i18n("Select All"));
    select_all_button.set_tooltip_text(Some(&i18n("Select all items (Ctrl+A)")));
    select_all_button.set_action_name(Some("win.select-all"));
    select_all_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Select all connections",
    ))]);
    bar.append(&select_all_button);

    let clear_button = Button::with_label(&i18n("Clear"));
    clear_button.set_tooltip_text(Some(&i18n("Clear selection (Escape)")));
    clear_button.set_action_name(Some("win.clear-selection"));
    clear_button.update_property(&[gtk4::accessible::Property::Label(&i18n("Clear selection"))]);
    bar.append(&clear_button);

    let delete_button = Button::with_label(&i18n("Delete"));
    delete_button.set_tooltip_text(Some(&i18n("Delete all selected items")));
    delete_button.set_action_name(Some("win.delete-selected"));
    delete_button.add_css_class("destructive-action");
    delete_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Delete selected connections",
    ))]);
    bar.append(&delete_button);

    bar
}

/// Creates the sidebar bottom toolbar with secondary actions
///
/// Layout: [Group Ops] [History] [A-Z Sort] [Recent] [Import] [Export] [KeePass]
#[must_use]
pub fn create_sidebar_bottom_toolbar() -> (GtkBox, Button) {
    let toolbar = GtkBox::new(Orientation::Horizontal, 4);
    toolbar.set_margin_start(8);
    toolbar.set_margin_end(8);
    toolbar.set_margin_top(6);
    toolbar.set_margin_bottom(6);
    toolbar.set_halign(gtk4::Align::Center);

    let group_ops_button = Button::from_icon_name("view-list-symbolic");
    group_ops_button.set_tooltip_text(Some(&i18n("Group Operations Mode")));
    group_ops_button.set_action_name(Some("win.group-operations"));
    group_ops_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Enable group operations mode for multi-select",
    ))]);
    toolbar.append(&group_ops_button);

    let history_button = Button::from_icon_name("document-open-recent-symbolic");
    history_button.set_tooltip_text(Some(&i18n("Connection History")));
    history_button.set_action_name(Some("win.show-history"));
    history_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "View connection history",
    ))]);
    toolbar.append(&history_button);

    let sort_button = Button::from_icon_name("view-sort-ascending-symbolic");
    sort_button.set_tooltip_text(Some(&i18n("Sort Alphabetically")));
    sort_button.set_action_name(Some("win.sort-connections"));
    sort_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Sort connections alphabetically",
    ))]);
    toolbar.append(&sort_button);

    let sort_recent_button = Button::from_icon_name("document-open-recent-symbolic");
    sort_recent_button.set_tooltip_text(Some(&i18n("Sort by Recent Usage")));
    sort_recent_button.set_action_name(Some("win.sort-recent"));
    sort_recent_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Sort connections by recent usage",
    ))]);
    toolbar.append(&sort_recent_button);

    let import_button = Button::from_icon_name("document-open-symbolic");
    import_button.set_tooltip_text(Some(&i18n("Import Connections (Ctrl+I)")));
    import_button.set_action_name(Some("win.import"));
    import_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Import connections from external sources",
    ))]);
    toolbar.append(&import_button);

    let export_button = Button::from_icon_name("document-save-symbolic");
    export_button.set_tooltip_text(Some(&i18n("Export Connections")));
    export_button.set_action_name(Some("win.export"));
    export_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Export connections to file",
    ))]);
    toolbar.append(&export_button);

    let keepass_button = Button::from_icon_name("dialog-password-symbolic");
    keepass_button.set_tooltip_text(Some(&i18n("Open Password Vault")));
    keepass_button.set_action_name(Some("win.open-keepass"));
    keepass_button.add_css_class("keepass-button");
    keepass_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Open password vault for credential management",
    ))]);
    toolbar.append(&keepass_button);

    (toolbar, keepass_button)
}
