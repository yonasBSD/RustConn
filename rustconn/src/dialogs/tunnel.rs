//! SSH Tunnel Manager window and Add/Edit tunnel dialog
//!
//! Provides a standalone window for managing SSH port-forwarding tunnels
//! that run independently of terminal sessions. Each tunnel references
//! an existing SSH connection for host/key/password configuration.

use crate::i18n::i18n;
use crate::state::{SharedAppState, with_state, with_state_mut};
use crate::window::SharedTunnelManager;
use adw::prelude::*;
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use rustconn_core::models::{
    Connection, PortForward, PortForwardDirection, ProtocolConfig, StandaloneTunnel,
};
use std::cell::RefCell;
use std::rc::Rc;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Tunnel Manager Window
// ---------------------------------------------------------------------------

/// Standalone window for managing SSH tunnels
pub struct TunnelManagerWindow {
    window: adw::Window,
    state: SharedAppState,
    tunnel_manager: SharedTunnelManager,
    content_stack: gtk4::Stack,
    active_group: Rc<RefCell<adw::PreferencesGroup>>,
    stopped_group: Rc<RefCell<adw::PreferencesGroup>>,
    prefs_page: adw::PreferencesPage,
}

impl TunnelManagerWindow {
    /// Creates a new tunnel manager window
    #[must_use]
    pub fn new(
        parent: Option<&gtk4::Window>,
        state: SharedAppState,
        tunnel_manager: SharedTunnelManager,
    ) -> Self {
        let window = adw::Window::builder()
            .title(i18n("SSH Tunnels"))
            .modal(true)
            .default_width(600)
            .default_height(500)
            .build();

        if let Some(p) = parent {
            window.set_transient_for(Some(p));
        }

        window.set_size_request(400, 350);

        // Header bar with add button
        let header = adw::HeaderBar::new();

        let add_button = gtk4::Button::from_icon_name("list-add-symbolic");
        add_button.add_css_class("flat");
        add_button.set_tooltip_text(Some(&i18n("Add Tunnel")));
        add_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
            "Add a new SSH tunnel",
        ))]);
        header.pack_start(&add_button);

        // Content stack: empty state vs tunnel list
        let content_stack = gtk4::Stack::new();
        content_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);

        // Empty state
        let empty_page = adw::StatusPage::builder()
            .icon_name("network-transmit-symbolic")
            .title(i18n("No Tunnels Configured"))
            .description(i18n(
                "SSH tunnels forward ports through encrypted connections",
            ))
            .build();

        let empty_add_button = gtk4::Button::builder()
            .label(i18n("Add Tunnel"))
            .halign(gtk4::Align::Center)
            .css_classes(["suggested-action", "pill"])
            .build();
        empty_page.set_child(Some(&empty_add_button));

        content_stack.add_named(&empty_page, Some("empty"));

        // Tunnel list
        let clamp = adw::Clamp::builder()
            .maximum_size(600)
            .tightening_threshold(400)
            .build();

        let scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .build();

        let prefs_page = adw::PreferencesPage::new();

        let active_group = Rc::new(RefCell::new(
            adw::PreferencesGroup::builder()
                .title(i18n("Active"))
                .build(),
        ));
        prefs_page.add(&*active_group.borrow());

        let stopped_group = Rc::new(RefCell::new(
            adw::PreferencesGroup::builder()
                .title(i18n("Stopped"))
                .build(),
        ));
        prefs_page.add(&*stopped_group.borrow());

        scroll.set_child(Some(&prefs_page));
        clamp.set_child(Some(&scroll));
        content_stack.add_named(&clamp, Some("list"));

        // Assemble toolbar view
        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&content_stack));
        window.set_content(Some(&toolbar_view));

        let manager = Self {
            window,
            state,
            tunnel_manager,
            content_stack,
            active_group,
            stopped_group,
            prefs_page,
        };

        // Wire up add buttons
        {
            let state_c = manager.state.clone();
            let window_c = manager.window.clone();
            let tm_c = manager.tunnel_manager.clone();
            let active_g = manager.active_group.clone();
            let stopped_g = manager.stopped_group.clone();
            let stack_c = manager.content_stack.clone();
            let page_c = manager.prefs_page.clone();
            add_button.connect_clicked(move |_| {
                show_add_edit_dialog(
                    &window_c, &state_c, None, &tm_c, &active_g, &stopped_g, &stack_c, &page_c,
                );
            });
        }
        {
            let state_c = manager.state.clone();
            let window_c = manager.window.clone();
            let tm_c = manager.tunnel_manager.clone();
            let active_g = manager.active_group.clone();
            let stopped_g = manager.stopped_group.clone();
            let stack_c = manager.content_stack.clone();
            let page_c = manager.prefs_page.clone();
            empty_add_button.connect_clicked(move |_| {
                show_add_edit_dialog(
                    &window_c, &state_c, None, &tm_c, &active_g, &stopped_g, &stack_c, &page_c,
                );
            });
        }

        manager.refresh_tunnel_list();
        manager
    }

    /// Refreshes the tunnel list from state
    pub fn refresh_tunnel_list(&self) {
        // Remove old groups and create fresh ones
        self.prefs_page.remove(&*self.active_group.borrow());
        self.prefs_page.remove(&*self.stopped_group.borrow());

        let new_active = adw::PreferencesGroup::builder()
            .title(i18n("Active"))
            .build();
        let new_stopped = adw::PreferencesGroup::builder()
            .title(i18n("Stopped"))
            .build();

        self.prefs_page.add(&new_active);
        self.prefs_page.add(&new_stopped);

        *self.active_group.borrow_mut() = new_active;
        *self.stopped_group.borrow_mut() = new_stopped;

        let tunnels = with_state(&self.state, |s| s.settings().standalone_tunnels.clone());
        let connections = with_state(&self.state, |s| {
            s.list_connections()
                .into_iter()
                .cloned()
                .collect::<Vec<_>>()
        });

        if tunnels.is_empty() {
            self.content_stack.set_visible_child_name("empty");
            return;
        }

        self.content_stack.set_visible_child_name("list");

        let ctx = Rc::new(TunnelRowContext {
            window: self.window.clone(),
            state: self.state.clone(),
            tunnel_manager: self.tunnel_manager.clone(),
            active_group: self.active_group.clone(),
            stopped_group: self.stopped_group.clone(),
            content_stack: self.content_stack.clone(),
            prefs_page: self.prefs_page.clone(),
        });

        let tm = self.tunnel_manager.borrow();
        for tunnel in &tunnels {
            let is_running = tm.is_running(tunnel.id);
            let row = build_tunnel_row(tunnel, &connections, is_running);

            // Wire up edit/delete/start/stop buttons in the expanded content
            wire_tunnel_row_actions(&row, tunnel, &ctx);

            if is_running {
                self.active_group.borrow().add(&row);
            } else {
                self.stopped_group.borrow().add(&row);
            }
        }
    }

    /// Presents the tunnel manager window
    pub fn present(&self) {
        self.window.present();
    }
}

// ---------------------------------------------------------------------------
// Helper: build a single tunnel ExpanderRow
// ---------------------------------------------------------------------------

/// Context for wiring tunnel row actions (avoids >6 params)
struct TunnelRowContext {
    window: adw::Window,
    state: SharedAppState,
    tunnel_manager: SharedTunnelManager,
    active_group: Rc<RefCell<adw::PreferencesGroup>>,
    stopped_group: Rc<RefCell<adw::PreferencesGroup>>,
    content_stack: gtk4::Stack,
    prefs_page: adw::PreferencesPage,
}

/// Builds an `adw::ExpanderRow` for a single tunnel definition
fn build_tunnel_row(
    tunnel: &StandaloneTunnel,
    connections: &[Connection],
    is_running: bool,
) -> adw::ExpanderRow {
    let summary = if tunnel.forwards.is_empty() {
        i18n("No port forwards configured")
    } else {
        tunnel.forwards_summary()
    };

    let row = adw::ExpanderRow::builder()
        .title(&tunnel.name)
        .subtitle(&summary)
        .build();

    // Status icon: green = running, gray = stopped
    let status_icon = gtk4::Image::from_icon_name("radio-symbolic");
    if is_running {
        status_icon.add_css_class("success");
    } else {
        status_icon.add_css_class("dim-label");
    }
    row.add_prefix(&status_icon);

    // Start/Stop toggle button (suffix)
    let (icon_name, tooltip, a11y_label) = if is_running {
        (
            "media-playback-stop-symbolic",
            i18n("Stop Tunnel"),
            i18n("Stop tunnel"),
        )
    } else {
        (
            "media-playback-start-symbolic",
            i18n("Start Tunnel"),
            i18n("Start tunnel"),
        )
    };

    let toggle_btn = gtk4::Button::from_icon_name(icon_name);
    toggle_btn.add_css_class("flat");
    toggle_btn.set_valign(gtk4::Align::Center);
    toggle_btn.set_tooltip_text(Some(&tooltip));
    toggle_btn.update_property(&[gtk4::accessible::Property::Label(&a11y_label)]);
    row.add_suffix(&toggle_btn);

    // Expanded content: connection name row
    let conn_name = connections
        .iter()
        .find(|c| c.id == tunnel.connection_id)
        .map(|c| {
            let user = c.username.as_deref().unwrap_or("?");
            // Escape markup-sensitive characters in connection details
            let escaped_name = glib::markup_escape_text(&c.name);
            let escaped_user = glib::markup_escape_text(user);
            let escaped_host = glib::markup_escape_text(&c.host);
            format!("{escaped_name} ({escaped_user}@{escaped_host})")
        })
        .unwrap_or_else(|| i18n("Unknown connection"));

    let conn_row = adw::ActionRow::builder()
        .title(i18n("SSH Connection"))
        .subtitle(&conn_name)
        .build();
    row.add_row(&conn_row);

    // Action buttons row
    let actions_row = adw::ActionRow::builder().title(i18n("Actions")).build();

    let edit_btn = gtk4::Button::from_icon_name("document-edit-symbolic");
    edit_btn.add_css_class("flat");
    edit_btn.set_valign(gtk4::Align::Center);
    edit_btn.set_tooltip_text(Some(&i18n("Edit Tunnel")));
    edit_btn.update_property(&[gtk4::accessible::Property::Label(&i18n("Edit this tunnel"))]);

    let delete_btn = gtk4::Button::from_icon_name("user-trash-symbolic");
    delete_btn.add_css_class("flat");
    delete_btn.add_css_class("destructive-action");
    delete_btn.set_valign(gtk4::Align::Center);
    delete_btn.set_tooltip_text(Some(&i18n("Delete Tunnel")));
    delete_btn.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Delete this tunnel",
    ))]);

    actions_row.add_suffix(&edit_btn);
    actions_row.add_suffix(&delete_btn);
    row.add_row(&actions_row);

    row
}

// ---------------------------------------------------------------------------
// Helper: wire edit/delete actions on a tunnel row
// ---------------------------------------------------------------------------

/// Wires edit and delete button actions for a tunnel expander row
fn wire_tunnel_row_actions(
    row: &adw::ExpanderRow,
    tunnel: &StandaloneTunnel,
    ctx: &Rc<TunnelRowContext>,
) {
    // Find the edit and delete buttons inside the actions row.
    // We walk the widget tree to find buttons by icon name.
    let tunnel_id = tunnel.id;
    let tunnel_clone = tunnel.clone();

    // Wire start/stop toggle button
    let is_running = ctx.tunnel_manager.borrow().is_running(tunnel_id);
    let toggle_icon = if is_running {
        "media-playback-stop-symbolic"
    } else {
        "media-playback-start-symbolic"
    };
    if let Some(toggle_btn) = find_button_in_expander(row, toggle_icon) {
        let ctx_c = ctx.clone();
        let tunnel_c = tunnel_clone.clone();
        toggle_btn.connect_clicked(move |_| {
            let running = ctx_c.tunnel_manager.borrow().is_running(tunnel_c.id);
            if running {
                // Stop the tunnel
                if let Err(e) = ctx_c.tunnel_manager.borrow_mut().stop(tunnel_c.id) {
                    tracing::warn!(tunnel = %tunnel_c.name, %e, "Failed to stop tunnel");
                }
            } else {
                // Start the tunnel — find the connection from state
                let connections = with_state(&ctx_c.state, |s| {
                    s.list_connections()
                        .into_iter()
                        .cloned()
                        .collect::<Vec<_>>()
                });
                if let Some(conn) = connections.iter().find(|c| c.id == tunnel_c.connection_id) {
                    // Resolve cached password for the connection
                    let cached_pw: Option<secrecy::SecretString> = with_state(&ctx_c.state, |s| {
                        s.get_cached_credentials(tunnel_c.connection_id)
                            .and_then(|c| {
                                use secrecy::ExposeSecret;
                                let pw = c.password.expose_secret();
                                if pw.is_empty() {
                                    None
                                } else {
                                    Some(c.password.clone())
                                }
                            })
                    });
                    if let Err(e) = ctx_c.tunnel_manager.borrow_mut().start(
                        &tunnel_c,
                        conn,
                        cached_pw.as_ref(),
                        &[],
                    ) {
                        tracing::warn!(tunnel = %tunnel_c.name, %e, "Failed to start tunnel");
                    }
                } else {
                    tracing::warn!(
                        tunnel = %tunnel_c.name,
                        connection_id = %tunnel_c.connection_id,
                        "SSH connection not found for tunnel"
                    );
                }
            }
            refresh_from_context(&ctx_c);
        });
    }

    if let Some(edit_btn) = find_button_in_expander(row, "document-edit-symbolic") {
        let ctx_c = ctx.clone();
        let tunnel_c = tunnel_clone.clone();
        edit_btn.connect_clicked(move |_| {
            show_add_edit_dialog(
                &ctx_c.window,
                &ctx_c.state,
                Some(&tunnel_c),
                &ctx_c.tunnel_manager,
                &ctx_c.active_group,
                &ctx_c.stopped_group,
                &ctx_c.content_stack,
                &ctx_c.prefs_page,
            );
        });
    }

    if let Some(delete_btn) = find_button_in_expander(row, "user-trash-symbolic") {
        let ctx_c = ctx.clone();
        delete_btn.connect_clicked(move |_| {
            delete_tunnel(tunnel_id, &ctx_c);
        });
    }
}

/// Searches for a button with a specific icon name inside an expander row's widget tree
fn find_button_in_expander(row: &adw::ExpanderRow, icon_name: &str) -> Option<gtk4::Button> {
    find_button_recursive(&row.clone().upcast::<gtk4::Widget>(), icon_name)
}

fn find_button_recursive(widget: &gtk4::Widget, icon_name: &str) -> Option<gtk4::Button> {
    if let Some(btn) = widget.downcast_ref::<gtk4::Button>()
        && btn.icon_name().as_deref() == Some(icon_name)
    {
        return Some(btn.clone());
    }
    let mut child = widget.first_child();
    while let Some(c) = child {
        if let Some(found) = find_button_recursive(&c, icon_name) {
            return Some(found);
        }
        child = c.next_sibling();
    }
    None
}

// ---------------------------------------------------------------------------
// Delete tunnel
// ---------------------------------------------------------------------------

/// Shows a confirmation dialog before deleting a tunnel (GNOME HIG: destructive actions)
fn delete_tunnel(tunnel_id: Uuid, ctx: &Rc<TunnelRowContext>) {
    // Look up the tunnel name for the confirmation message
    let tunnel_name = with_state(&ctx.state, |s| {
        s.settings()
            .standalone_tunnels
            .iter()
            .find(|t| t.id == tunnel_id)
            .map(|t| t.name.clone())
            .unwrap_or_default()
    });

    let confirm = adw::AlertDialog::builder()
        .heading(i18n("Delete Tunnel?"))
        .body(crate::i18n::i18n_f(
            "Tunnel \"{}\" will be permanently removed.",
            &[&tunnel_name],
        ))
        .build();

    confirm.add_response("cancel", &i18n("Cancel"));
    confirm.add_response("delete", &i18n("Delete"));
    confirm.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
    confirm.set_default_response(Some("cancel"));
    confirm.set_close_response("cancel");

    let ctx_c = ctx.clone();
    confirm.connect_response(None, move |_, response| {
        if response != "delete" {
            return;
        }

        // Stop the tunnel if it's running
        if ctx_c.tunnel_manager.borrow().is_running(tunnel_id) {
            let _ = ctx_c.tunnel_manager.borrow_mut().stop(tunnel_id);
        }

        with_state_mut(&ctx_c.state, |s| {
            s.settings_mut()
                .standalone_tunnels
                .retain(|t| t.id != tunnel_id);
            if let Err(e) = s.save_settings() {
                tracing::error!(%e, "Failed to save settings after tunnel delete");
            }
        });
        refresh_from_context(&ctx_c);
    });

    confirm.present(Some(&ctx.window));
}

/// Refreshes the tunnel list using a `TunnelRowContext`
fn refresh_from_context(ctx: &TunnelRowContext) {
    // Remove old groups and create fresh ones
    ctx.prefs_page.remove(&*ctx.active_group.borrow());
    ctx.prefs_page.remove(&*ctx.stopped_group.borrow());

    let new_active = adw::PreferencesGroup::builder()
        .title(i18n("Active"))
        .build();
    let new_stopped = adw::PreferencesGroup::builder()
        .title(i18n("Stopped"))
        .build();

    ctx.prefs_page.add(&new_active);
    ctx.prefs_page.add(&new_stopped);

    *ctx.active_group.borrow_mut() = new_active;
    *ctx.stopped_group.borrow_mut() = new_stopped;

    let tunnels = with_state(&ctx.state, |s| s.settings().standalone_tunnels.clone());
    let connections = with_state(&ctx.state, |s| {
        s.list_connections()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
    });

    if tunnels.is_empty() {
        ctx.content_stack.set_visible_child_name("empty");
        return;
    }

    ctx.content_stack.set_visible_child_name("list");

    let rc_ctx = Rc::new(TunnelRowContext {
        window: ctx.window.clone(),
        state: ctx.state.clone(),
        tunnel_manager: ctx.tunnel_manager.clone(),
        active_group: ctx.active_group.clone(),
        stopped_group: ctx.stopped_group.clone(),
        content_stack: ctx.content_stack.clone(),
        prefs_page: ctx.prefs_page.clone(),
    });

    let tm = ctx.tunnel_manager.borrow();
    for tunnel in &tunnels {
        let is_running = tm.is_running(tunnel.id);
        let row = build_tunnel_row(tunnel, &connections, is_running);
        wire_tunnel_row_actions(&row, tunnel, &rc_ctx);
        if is_running {
            ctx.active_group.borrow().add(&row);
        } else {
            ctx.stopped_group.borrow().add(&row);
        }
    }
}

// ---------------------------------------------------------------------------
// Add / Edit Tunnel Dialog
// ---------------------------------------------------------------------------

/// Shows the add/edit tunnel dialog as a modal `adw::Dialog`
///
/// When `existing` is `Some`, the dialog is pre-populated for editing.
/// When `None`, a blank dialog is shown for creating a new tunnel.
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn show_add_edit_dialog(
    parent: &adw::Window,
    state: &SharedAppState,
    existing: Option<&StandaloneTunnel>,
    tunnel_manager: &SharedTunnelManager,
    active_group: &Rc<RefCell<adw::PreferencesGroup>>,
    stopped_group: &Rc<RefCell<adw::PreferencesGroup>>,
    content_stack: &gtk4::Stack,
    prefs_page: &adw::PreferencesPage,
) {
    let is_edit = existing.is_some();
    let dialog_title = if is_edit {
        i18n("Edit Tunnel")
    } else {
        i18n("Add Tunnel")
    };

    let dialog = adw::Dialog::builder()
        .title(&dialog_title)
        .content_width(500)
        .content_height(600)
        .build();

    let (header, cancel_btn, save_btn) = crate::dialogs::widgets::dialog_header("Cancel", "Save");

    // Save button starts disabled for new tunnels (name is empty)
    if !is_edit {
        save_btn.set_sensitive(false);
    }

    let clamp = adw::Clamp::builder()
        .maximum_size(600)
        .tightening_threshold(400)
        .build();

    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(true)
        .build();

    let page = adw::PreferencesPage::new();

    // === General Group ===
    let general_group = adw::PreferencesGroup::builder()
        .title(i18n("General"))
        .build();

    let name_row = adw::EntryRow::builder().title(i18n("Tunnel Name")).build();
    general_group.add(&name_row);

    // Enable/disable Save button based on tunnel name (inline validation)
    {
        let save_btn_c = save_btn.clone();
        name_row.connect_changed(move |entry| {
            let has_name = !entry.text().trim().is_empty();
            save_btn_c.set_sensitive(has_name);
        });
    }

    // SSH connection combo
    let ssh_connections = with_state(state, |s| {
        s.list_connections()
            .into_iter()
            .filter(|c| matches!(c.protocol_config, ProtocolConfig::Ssh(_)))
            .cloned()
            .collect::<Vec<_>>()
    });

    let conn_labels: Vec<String> = ssh_connections
        .iter()
        .map(|c| {
            let user = c.username.as_deref().unwrap_or("?");
            // Escape markup-sensitive characters (<, >, &) in connection names
            // to prevent GTK Pango markup parsing errors
            let escaped_name = glib::markup_escape_text(&c.name);
            let escaped_user = glib::markup_escape_text(user);
            let escaped_host = glib::markup_escape_text(&c.host);
            format!("{escaped_name} ({escaped_user}@{escaped_host})")
        })
        .collect();

    let conn_string_list =
        gtk4::StringList::new(&conn_labels.iter().map(String::as_str).collect::<Vec<_>>());

    let connection_combo = adw::ComboRow::builder()
        .title(i18n("SSH Connection"))
        .model(&conn_string_list)
        .build();
    general_group.add(&connection_combo);

    // "New SSH Connection" button below the combo
    let new_conn_btn = gtk4::Button::builder()
        .label(i18n("New SSH Connection"))
        .halign(gtk4::Align::Start)
        .margin_top(6)
        .css_classes(["flat"])
        .build();
    new_conn_btn.set_tooltip_text(Some(&i18n("Create a new SSH connection")));
    new_conn_btn.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Create a new SSH connection for this tunnel",
    ))]);

    // Open ConnectionDialog for creating a new SSH connection.
    // After save, refresh the SSH connection combo to include the new one.
    {
        let state_c = state.clone();
        let combo_c = connection_combo.clone();
        let parent_c = parent.clone();
        let ssh_conns = Rc::new(RefCell::new(ssh_connections.clone()));
        new_conn_btn.connect_clicked(move |_| {
            let dialog = crate::dialogs::ConnectionDialog::new(
                Some(&parent_c.clone().upcast()),
                state_c.clone(),
            );
            dialog.setup_key_file_chooser(Some(&parent_c.clone().upcast()));

            // Set available groups and connections
            if let Ok(state_ref) = state_c.try_borrow() {
                let mut groups: Vec<_> = state_ref.list_groups().into_iter().cloned().collect();
                groups.sort_by_key(|a| a.name.to_lowercase());
                dialog.set_groups(&groups);
                let connections: Vec<_> =
                    state_ref.list_connections().into_iter().cloned().collect();
                dialog.set_connections(&connections);
                dialog.set_preferred_backend(state_ref.settings().secrets.preferred_backend);
                dialog.set_global_variables(&state_ref.settings().global_variables);
            }

            dialog.connect_password_visibility_toggle();
            dialog.connect_password_source_visibility();
            dialog.update_password_row_visibility();

            // Set up password load button
            if let Ok(state_ref) = state_c.try_borrow() {
                use secrecy::ExposeSecret;
                let settings = state_ref.settings();
                let groups: Vec<rustconn_core::models::ConnectionGroup> =
                    state_ref.list_groups().iter().cloned().cloned().collect();
                dialog.connect_password_load_button_with_groups(
                    settings.secrets.kdbx_enabled,
                    settings.secrets.kdbx_path.clone(),
                    settings
                        .secrets
                        .kdbx_password
                        .as_ref()
                        .map(|p| p.expose_secret().to_string()),
                    settings.secrets.kdbx_key_file.clone(),
                    groups,
                    settings.secrets.clone(),
                );
            }

            // Pre-select SSH protocol by setting a minimal SSH connection
            dialog.set_connection(&rustconn_core::Connection::new(
                String::new(),
                String::new(),
                22,
                ProtocolConfig::Ssh(rustconn_core::models::SshConfig::default()),
            ));

            let state_save = state_c.clone();
            let combo_save = combo_c.clone();
            let ssh_conns_save = ssh_conns.clone();
            let parent_save = parent_c.clone();
            dialog.run(move |result| {
                if let Some(dialog_result) = result {
                    let conn = dialog_result.connection;
                    let password = dialog_result.password;

                    if let Ok(mut state_mut) = state_save.try_borrow_mut() {
                        let conn_name = conn.name.clone();
                        let conn_host = conn.host.clone();
                        let conn_username = conn.username.clone();
                        let password_source = conn.password_source.clone();
                        let protocol = conn.protocol;

                        match state_mut.create_connection(conn) {
                            Ok(conn_id) => {
                                // Save password to vault if needed
                                if password_source == rustconn_core::models::PasswordSource::Vault
                                    && let Some(pwd) = password
                                {
                                    use secrecy::ExposeSecret;
                                    let settings = state_mut.settings().clone();
                                    let groups: Vec<_> =
                                        state_mut.list_groups().into_iter().cloned().collect();
                                    let conn_for_path = state_mut.get_connection(conn_id).cloned();
                                    let username = conn_username.unwrap_or_default();

                                    crate::state::save_password_to_vault(
                                        &settings,
                                        &groups,
                                        conn_for_path.as_ref(),
                                        &conn_name,
                                        &conn_host,
                                        protocol,
                                        &username,
                                        pwd.expose_secret(),
                                        conn_id,
                                    );
                                }

                                // Refresh the SSH connection combo
                                let new_ssh_conns: Vec<Connection> = state_mut
                                    .list_connections()
                                    .into_iter()
                                    .filter(|c| matches!(c.protocol_config, ProtocolConfig::Ssh(_)))
                                    .cloned()
                                    .collect();

                                let labels: Vec<String> = new_ssh_conns
                                    .iter()
                                    .map(|c| {
                                        let user = c.username.as_deref().unwrap_or("?");
                                        let en = glib::markup_escape_text(&c.name);
                                        let eu = glib::markup_escape_text(user);
                                        let eh = glib::markup_escape_text(&c.host);
                                        format!("{en} ({eu}@{eh})")
                                    })
                                    .collect();

                                let label_refs: Vec<&str> =
                                    labels.iter().map(String::as_str).collect();
                                let new_model = gtk4::StringList::new(&label_refs);
                                combo_save.set_model(Some(&new_model));

                                // Select the newly created connection
                                if let Some(idx) =
                                    new_ssh_conns.iter().position(|c| c.id == conn_id)
                                {
                                    combo_save.set_selected(idx as u32);
                                }

                                *ssh_conns_save.borrow_mut() = new_ssh_conns;
                            }
                            Err(e) => {
                                crate::alert::show_error(
                                    parent_save.upcast_ref::<gtk4::Window>(),
                                    &i18n("Error Creating Connection"),
                                    &e,
                                );
                            }
                        }
                    }
                }
            });
        });
    }

    let new_conn_row = adw::ActionRow::new();
    new_conn_row.add_suffix(&new_conn_btn);
    general_group.add(&new_conn_row);

    page.add(&general_group);

    // === Port Forwards Group ===
    let forwards_group = adw::PreferencesGroup::builder()
        .title(i18n("Port Forwards"))
        .build();

    let forwards_list: Rc<RefCell<Vec<ForwardRowWidgets>>> = Rc::new(RefCell::new(Vec::new()));

    let add_forward_btn = gtk4::Button::builder()
        .label(i18n("Add Forward"))
        .halign(gtk4::Align::Start)
        .css_classes(["flat"])
        .build();
    add_forward_btn.set_tooltip_text(Some(&i18n("Add a port forwarding rule")));
    add_forward_btn.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Add a new port forwarding rule",
    ))]);

    page.add(&forwards_group);

    // Add forward button in its own group to keep it outside the list
    let add_fwd_group = adw::PreferencesGroup::new();
    let add_fwd_row = adw::ActionRow::new();
    add_fwd_row.add_suffix(&add_forward_btn);
    add_fwd_group.add(&add_fwd_row);
    page.add(&add_fwd_group);

    // === Options Group ===
    let options_group = adw::PreferencesGroup::builder()
        .title(i18n("Options"))
        .build();

    let auto_start_row = adw::SwitchRow::builder()
        .title(i18n("Auto-start on launch"))
        .subtitle(i18n("Start this tunnel when the application opens"))
        .build();
    options_group.add(&auto_start_row);

    let auto_reconnect_row = adw::SwitchRow::builder()
        .title(i18n("Auto-reconnect on failure"))
        .subtitle(i18n("Automatically restart if the tunnel disconnects"))
        .build();
    options_group.add(&auto_reconnect_row);

    page.add(&options_group);

    // Populate fields if editing
    let editing_id: Rc<RefCell<Option<Uuid>>> = Rc::new(RefCell::new(None));

    if let Some(tunnel) = existing {
        *editing_id.borrow_mut() = Some(tunnel.id);
        name_row.set_text(&tunnel.name);
        auto_start_row.set_active(tunnel.auto_start);
        auto_reconnect_row.set_active(tunnel.auto_reconnect);

        // Select the matching SSH connection in the combo
        if let Some(idx) = ssh_connections
            .iter()
            .position(|c| c.id == tunnel.connection_id)
        {
            connection_combo.set_selected(idx as u32);
        }

        // Populate existing forwards
        for fwd in &tunnel.forwards {
            let row_widgets = add_forward_row(&forwards_group, &forwards_list, Some(fwd));
            forwards_list.borrow_mut().push(row_widgets);
        }
    }

    // Wire "Add Forward" button
    {
        let fwd_group_c = forwards_group.clone();
        let fwd_list_c = forwards_list.clone();
        add_forward_btn.connect_clicked(move |_| {
            let row_widgets = add_forward_row(&fwd_group_c, &fwd_list_c, None);
            fwd_list_c.borrow_mut().push(row_widgets);
        });
    }

    scroll.set_child(Some(&page));
    clamp.set_child(Some(&scroll));

    let toolbar_view = adw::ToolbarView::new();
    toolbar_view.add_top_bar(&header);
    toolbar_view.set_content(Some(&clamp));
    dialog.set_child(Some(&toolbar_view));

    // Cancel
    let dialog_c = dialog.clone();
    cancel_btn.connect_clicked(move |_| {
        dialog_c.close();
    });

    // Save
    let dialog_c = dialog.clone();
    let state_c = state.clone();
    let ssh_conns = ssh_connections;
    let active_g = active_group.clone();
    let stopped_g = stopped_group.clone();
    let stack_c = content_stack.clone();
    let page_c = prefs_page.clone();
    let parent_window = parent.clone();
    let tm_c = tunnel_manager.clone();
    save_btn.connect_clicked(move |_| {
        let tunnel_name = name_row.text().to_string();
        // Save button is disabled when name is empty, but guard defensively
        if tunnel_name.trim().is_empty() {
            return;
        }

        let selected_idx = connection_combo.selected() as usize;
        let Some(conn) = ssh_conns.get(selected_idx) else {
            return;
        };

        // Collect forwards from the UI
        let forwards: Vec<PortForward> = forwards_list
            .borrow()
            .iter()
            .filter_map(|fw| fw.to_port_forward())
            .collect();

        let auto_start = auto_start_row.is_active();
        let auto_reconnect = auto_reconnect_row.is_active();

        let editing = *editing_id.borrow();

        with_state_mut(&state_c, |s| {
            if let Some(id) = editing {
                // Update existing tunnel
                if let Some(tunnel) = s
                    .settings_mut()
                    .standalone_tunnels
                    .iter_mut()
                    .find(|t| t.id == id)
                {
                    tunnel.name.clone_from(&tunnel_name);
                    tunnel.connection_id = conn.id;
                    tunnel.forwards.clone_from(&forwards);
                    tunnel.auto_start = auto_start;
                    tunnel.auto_reconnect = auto_reconnect;
                }
            } else {
                // Create new tunnel
                let mut tunnel = StandaloneTunnel::new(tunnel_name.clone(), conn.id);
                tunnel.forwards.clone_from(&forwards);
                tunnel.auto_start = auto_start;
                tunnel.auto_reconnect = auto_reconnect;
                s.settings_mut().standalone_tunnels.push(tunnel);
            }

            if let Err(e) = s.save_settings() {
                tracing::error!(%e, "Failed to save settings after tunnel save");
            }
        });

        dialog_c.close();

        // Refresh the tunnel list
        let ctx = TunnelRowContext {
            window: parent_window.clone(),
            state: state_c.clone(),
            tunnel_manager: tm_c.clone(),
            active_group: active_g.clone(),
            stopped_group: stopped_g.clone(),
            content_stack: stack_c.clone(),
            prefs_page: page_c.clone(),
        };
        refresh_from_context(&ctx);
    });

    dialog.present(Some(parent));
}

// ---------------------------------------------------------------------------
// Port Forward Row Widgets
// ---------------------------------------------------------------------------

/// Holds references to widgets in a single port-forward row
struct ForwardRowWidgets {
    row: adw::ExpanderRow,
    direction_dropdown: gtk4::DropDown,
    local_port_spin: adw::SpinRow,
    remote_host_entry: adw::EntryRow,
    remote_port_spin: adw::SpinRow,
}

impl ForwardRowWidgets {
    /// Extracts a `PortForward` from the current widget values
    fn to_port_forward(&self) -> Option<PortForward> {
        let local_port = self.local_port_spin.value() as u16;
        if local_port == 0 {
            return None;
        }

        let direction = match self.direction_dropdown.selected() {
            0 => PortForwardDirection::Local,
            1 => PortForwardDirection::Remote,
            2 => PortForwardDirection::Dynamic,
            _ => PortForwardDirection::Local,
        };

        let remote_host = self.remote_host_entry.text().to_string();
        let remote_port = self.remote_port_spin.value() as u16;

        Some(PortForward {
            direction,
            local_port,
            remote_host,
            remote_port,
        })
    }
}

/// Adds a port-forward row to the forwards group and returns its widgets
fn add_forward_row(
    group: &adw::PreferencesGroup,
    forwards_list: &Rc<RefCell<Vec<ForwardRowWidgets>>>,
    existing: Option<&PortForward>,
) -> ForwardRowWidgets {
    let direction_items = [
        i18n("Local (-L)"),
        i18n("Remote (-R)"),
        i18n("Dynamic (-D)"),
    ];
    let direction_string_list = gtk4::StringList::new(
        &direction_items
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
    );

    let direction_dropdown = gtk4::DropDown::builder()
        .model(&direction_string_list)
        .valign(gtk4::Align::Center)
        .build();

    let summary = existing
        .map(|f| f.display_summary())
        .unwrap_or_else(|| i18n("New forward"));

    let expander = adw::ExpanderRow::builder().title(&summary).build();

    expander.add_suffix(&direction_dropdown);

    // Remove button
    let remove_btn = gtk4::Button::from_icon_name("edit-delete-symbolic");
    remove_btn.add_css_class("flat");
    remove_btn.add_css_class("destructive-action");
    remove_btn.set_valign(gtk4::Align::Center);
    remove_btn.set_tooltip_text(Some(&i18n("Remove Forward")));
    remove_btn.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Remove this port forwarding rule",
    ))]);
    expander.add_suffix(&remove_btn);

    // Local port
    let local_port_spin = adw::SpinRow::builder()
        .title(i18n("Local Port"))
        .adjustment(&gtk4::Adjustment::new(
            8080.0, 1.0, 65535.0, 1.0, 100.0, 0.0,
        ))
        .build();
    expander.add_row(&local_port_spin);

    // Remote host
    let remote_host_entry = adw::EntryRow::builder().title(i18n("Remote Host")).build();
    remote_host_entry.set_text("localhost");
    expander.add_row(&remote_host_entry);

    // Remote port
    let remote_port_spin = adw::SpinRow::builder()
        .title(i18n("Remote Port"))
        .adjustment(&gtk4::Adjustment::new(80.0, 0.0, 65535.0, 1.0, 100.0, 0.0))
        .build();
    expander.add_row(&remote_port_spin);

    // Populate from existing forward
    if let Some(fwd) = existing {
        let dir_idx = match fwd.direction {
            PortForwardDirection::Local => 0,
            PortForwardDirection::Remote => 1,
            PortForwardDirection::Dynamic => 2,
        };
        direction_dropdown.set_selected(dir_idx);
        local_port_spin.set_value(f64::from(fwd.local_port));
        remote_host_entry.set_text(&fwd.remote_host);
        remote_port_spin.set_value(f64::from(fwd.remote_port));
    }

    // Update expander title when direction or ports change
    {
        let expander_c = expander.clone();
        let dir_clone = direction_dropdown.clone();
        let lport_clone = local_port_spin.clone();
        let rhost_clone = remote_host_entry.clone();
        let rport_clone = remote_port_spin.clone();

        let update_title = Rc::new(move || {
            let dir = match dir_clone.selected() {
                0 => "L",
                1 => "R",
                2 => "D",
                _ => "?",
            };
            let lp = lport_clone.value() as u16;
            let rh = rhost_clone.text();
            let rp = rport_clone.value() as u16;
            let title = if dir == "D" {
                format!("D {lp} (SOCKS)")
            } else {
                format!("{dir} {lp} → {rh}:{rp}")
            };
            expander_c.set_title(&title);
        });

        let u1 = update_title.clone();
        direction_dropdown.connect_selected_notify(move |_| u1());

        let u2 = update_title.clone();
        local_port_spin.connect_changed(move |_| u2());

        let u3 = update_title.clone();
        remote_host_entry.connect_changed(move |_| u3());

        let u4 = update_title;
        remote_port_spin.connect_changed(move |_| u4());
    }

    // Wire remove button
    {
        let group_c = group.clone();
        let expander_c = expander.clone();
        let fwd_list_c = forwards_list.clone();
        remove_btn.connect_clicked(move |_| {
            group_c.remove(&expander_c);
            fwd_list_c.borrow_mut().retain(|fw| fw.row != expander_c);
        });
    }

    group.add(&expander);

    ForwardRowWidgets {
        row: expander,
        direction_dropdown,
        local_port_spin,
        remote_host_entry,
        remote_port_spin,
    }
}
