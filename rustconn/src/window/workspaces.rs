//! Workspace profile management UI integration
//!
//! Connects the workspace manager dialog to the application state and
//! provides "Save current" and "Open workspace" functionality.

use adw::prelude::*;
use gtk4::prelude::*;
use libadwaita as adw;

use crate::dialogs::WorkspaceManagerDialog;
use crate::i18n::{i18n, i18n_f};
use crate::state::SharedAppState;
use crate::toast::{ToastType, show_toast_on_window};
use crate::window::types::{SharedMonitoring, SharedNotebook, SharedSidebar};

use rustconn_core::models::{WorkspaceEntry, WorkspaceProfile};

/// Shows the workspace manager dialog
pub fn show_workspace_manager(
    window: &gtk4::Window,
    state: SharedAppState,
    notebook: SharedNotebook,
    sidebar: SharedSidebar,
    monitoring: SharedMonitoring,
) {
    let dialog = WorkspaceManagerDialog::new(None);

    // Provider: fetch workspace profiles from state
    let state_for_provider = state.clone();
    dialog.set_provider(move || {
        if let Ok(state_ref) = state_for_provider.try_borrow() {
            state_ref
                .list_workspace_profiles()
                .iter()
                .map(|ws| (ws.id, ws.name.clone(), ws.entry_count()))
                .collect()
        } else {
            Vec::new()
        }
    });

    // Open callback: connect all entries in the workspace, then restore the
    // saved split layout (if any) via the active window's split machinery.
    let state_for_open = state.clone();
    let notebook_for_open = notebook.clone();
    let sidebar_for_open = sidebar.clone();
    let monitoring_for_open = monitoring.clone();
    let window_for_open = window.downgrade();
    dialog.set_on_open(move |workspace_id| {
        let profile = if let Ok(state_ref) = state_for_open.try_borrow() {
            state_ref.get_workspace_profile(workspace_id).cloned()
        } else {
            None
        };
        if let Some(profile) = profile {
            for entry in &profile.entries {
                let _ = super::MainWindow::start_connection(
                    &state_for_open,
                    &notebook_for_open,
                    &sidebar_for_open,
                    &monitoring_for_open,
                    entry.connection_id,
                );
            }
            if let Some(win) = window_for_open.upgrade() {
                crate::split_view::apply_layout(&win, &profile.split_layout);
            }
        }
    });

    // Delete callback
    let state_for_delete = state.clone();
    let dialog_rc = std::rc::Rc::new(dialog);
    let dialog_for_delete = dialog_rc.clone();
    dialog_rc.set_on_delete(move |workspace_id| {
        if let Ok(mut state_ref) = state_for_delete.try_borrow_mut()
            && let Err(e) = state_ref.delete_workspace_profile(workspace_id)
        {
            tracing::warn!("Failed to delete workspace: {e}");
        }
        dialog_for_delete.refresh_list();
    });

    // Rename callback
    let state_for_rename = state.clone();
    let dialog_for_rename = dialog_rc.clone();
    dialog_rc.set_on_rename(move |workspace_id, new_name| {
        if let Ok(mut state_ref) = state_for_rename.try_borrow_mut()
            && let Err(e) = state_ref.rename_workspace_profile(workspace_id, new_name)
        {
            tracing::warn!("Failed to rename workspace: {e}");
        }
        dialog_for_rename.refresh_list();
    });

    // Save current callback
    let state_for_save = state.clone();
    let dialog_for_save = dialog_rc.clone();
    let window_weak = window.downgrade();
    dialog_rc.set_on_save_current(move || {
        if let Some(win) = window_weak.upgrade() {
            save_current_workspace(&state_for_save, &win);
        }
        dialog_for_save.refresh_list();
    });

    dialog_rc.refresh_list();
    dialog_rc.show(window.upcast_ref::<gtk4::Widget>());
}

/// Saves currently open sessions as a new workspace profile
fn save_current_workspace(state: &SharedAppState, window: &gtk4::Window) {
    // Collect active sessions from SessionManager
    let entries: Vec<WorkspaceEntry> = if let Ok(state_ref) = state.try_borrow() {
        state_ref
            .active_sessions()
            .iter()
            .enumerate()
            .map(|(i, session)| {
                WorkspaceEntry::new(
                    session.connection_id,
                    session.connection_name.clone(),
                    session.protocol.clone(),
                    session.session_type,
                    i,
                )
            })
            .collect()
    } else {
        Vec::new()
    };

    if entries.is_empty() {
        show_toast_on_window(window, &i18n("No active sessions to save"), ToastType::Info);
        return;
    }

    // Prompt for name
    let state_clone = state.clone();
    let entries_clone = entries;
    let window_weak = window.downgrade();

    let alert = adw::AlertDialog::new(
        Some(&i18n("Save Workspace")),
        Some(&i18n("Enter a name for this workspace profile:")),
    );
    alert.add_response("cancel", &i18n("Cancel"));
    alert.add_response("save", &i18n("Save"));
    alert.set_response_appearance("save", adw::ResponseAppearance::Suggested);
    alert.set_default_response(Some("save"));
    alert.set_close_response("cancel");

    let entry = gtk4::Entry::builder()
        .placeholder_text(i18n("Workspace name"))
        .activates_default(true)
        .build();
    alert.set_extra_child(Some(&entry));

    let entry_clone = entry.clone();
    alert.connect_response(None, move |_, response| {
        if response != "save" {
            return;
        }
        let name = entry_clone.text().to_string();
        let name = name.trim().to_string();
        if name.is_empty() {
            return;
        }

        let mut profile = WorkspaceProfile::new(&name);
        for e in &entries_clone {
            profile.add_entry(e.clone());
        }

        if let Ok(mut state_ref) = state_clone.try_borrow_mut() {
            match state_ref.create_workspace_profile(profile) {
                Ok(_) => {
                    if let Some(win) = window_weak.upgrade() {
                        let msg = i18n_f("Workspace '{}' saved", &[&name]);
                        show_toast_on_window(&win, &msg, ToastType::Success);
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to save workspace: {e}");
                    if let Some(win) = window_weak.upgrade() {
                        show_toast_on_window(
                            &win,
                            &i18n("Failed to save workspace"),
                            ToastType::Error,
                        );
                    }
                }
            }
        }
    });

    alert.present(Some(window));
}
