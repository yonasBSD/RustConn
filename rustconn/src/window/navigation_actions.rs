//! Navigation and group operations window actions
//!
//! Extracted from `window/mod.rs` to reduce module complexity.

use super::*;

impl MainWindow {
    pub(crate) fn setup_navigation_actions(
        &self,
        window: &adw::ApplicationWindow,
        terminal_notebook: &SharedNotebook,
        sidebar: &SharedSidebar,
        state: &SharedAppState,
    ) {
        // Focus sidebar action
        let focus_sidebar_action = gio::SimpleAction::new("focus-sidebar", None);
        let sidebar_clone = sidebar.clone();
        focus_sidebar_action.connect_activate(move |_, _| {
            sidebar_clone.list_view().grab_focus();
        });
        window.add_action(&focus_sidebar_action);

        // Focus terminal action
        let focus_terminal_action = gio::SimpleAction::new("focus-terminal", None);
        let notebook_clone = terminal_notebook.clone();
        focus_terminal_action.connect_activate(move |_, _| {
            if let Some(terminal) = notebook_clone.get_active_terminal() {
                terminal.grab_focus();
            }
        });
        window.add_action(&focus_terminal_action);

        // Next tab action
        let next_tab_action = gio::SimpleAction::new("next-tab", None);
        let notebook_clone = terminal_notebook.clone();
        next_tab_action.connect_activate(move |_, _| {
            let tab_view = notebook_clone.tab_view();
            let n_pages = tab_view.n_pages();
            if n_pages > 0
                && let Some(selected) = tab_view.selected_page()
            {
                let current_pos = tab_view.page_position(&selected);
                let next_pos = (current_pos + 1) % n_pages;
                let next_page = tab_view.nth_page(next_pos);
                tab_view.set_selected_page(&next_page);
            }
        });
        window.add_action(&next_tab_action);

        // Previous tab action
        let prev_tab_action = gio::SimpleAction::new("prev-tab", None);
        let notebook_clone = terminal_notebook.clone();
        prev_tab_action.connect_activate(move |_, _| {
            let tab_view = notebook_clone.tab_view();
            let n_pages = tab_view.n_pages();
            if n_pages > 0
                && let Some(selected) = tab_view.selected_page()
            {
                let current_pos = tab_view.page_position(&selected);
                let prev_pos = if current_pos == 0 {
                    n_pages - 1
                } else {
                    current_pos - 1
                };
                let prev_page = tab_view.nth_page(prev_pos);
                tab_view.set_selected_page(&prev_page);
            }
        });
        window.add_action(&prev_tab_action);

        // Tab overview action — opens the grid view of all tabs
        let tab_overview_action = gio::SimpleAction::new("tab-overview", None);
        let notebook_clone = terminal_notebook.clone();
        tab_overview_action.connect_activate(move |_, _| {
            notebook_clone.open_tab_overview();
        });
        window.add_action(&tab_overview_action);

        // Switch tab via command palette (% prefix)
        let switch_tab_action = gio::SimpleAction::new("switch-tab-palette", None);
        let window_weak = window.downgrade();
        let state_clone = state.clone();
        let sidebar_clone = sidebar.clone();
        let notebook_clone = terminal_notebook.clone();
        let monitoring_clone = self.monitoring.clone();
        switch_tab_action.connect_activate(move |_, _| {
            if let Some(win) = window_weak.upgrade() {
                Self::show_command_palette(
                    &win,
                    &state_clone,
                    &sidebar_clone,
                    &notebook_clone,
                    &monitoring_clone,
                    "%",
                );
            }
        });
        window.add_action(&switch_tab_action);

        // Toggle fullscreen action (stateful per GNOME HIG — menu shows checkmark)
        let toggle_fullscreen_action =
            gio::SimpleAction::new_stateful("toggle-fullscreen", None, &false.to_variant());
        let window_weak = window.downgrade();
        toggle_fullscreen_action.connect_activate(move |action, _| {
            if let Some(win) = window_weak.upgrade() {
                let is_fullscreen = win.is_fullscreen();
                if is_fullscreen {
                    win.unfullscreen();
                } else {
                    win.fullscreen();
                }
                action.set_state(&(!is_fullscreen).to_variant());
            }
        });
        window.add_action(&toggle_fullscreen_action);
    }

    /// Sets up group operations actions (select all, delete selected, etc.)
    pub(crate) fn setup_group_operations_actions(
        &self,
        window: &adw::ApplicationWindow,
        state: &SharedAppState,
        terminal_notebook: &SharedNotebook,
        sidebar: &SharedSidebar,
    ) {
        // Group operations action (toggle mode)
        let group_ops_action =
            gio::SimpleAction::new_stateful("group-operations", None, &false.to_variant());
        let sidebar_clone = sidebar.clone();
        group_ops_action.connect_activate(move |action, _| {
            let current = action
                .state()
                .and_then(|v| v.get::<bool>())
                .unwrap_or(false);
            action.set_state(&(!current).to_variant());
            Self::toggle_group_operations_mode(&sidebar_clone, !current);
        });
        window.add_action(&group_ops_action);

        // Select all action
        let select_all_action = gio::SimpleAction::new("select-all", None);
        let sidebar_clone = sidebar.clone();
        select_all_action.connect_activate(move |_, _| {
            if sidebar_clone.is_group_operations_mode() {
                sidebar_clone.select_all();
            }
        });
        window.add_action(&select_all_action);

        // Clear selection action
        let clear_selection_action = gio::SimpleAction::new("clear-selection", None);
        let sidebar_clone = sidebar.clone();
        clear_selection_action.connect_activate(move |_, _| {
            sidebar_clone.clear_selection();
        });
        window.add_action(&clear_selection_action);

        // Delete selected action
        let delete_selected_action = gio::SimpleAction::new("delete-selected", None);
        let window_weak = window.downgrade();
        let state_clone = state.clone();
        let sidebar_clone = sidebar.clone();
        delete_selected_action.connect_activate(move |_, _| {
            if let Some(win) = window_weak.upgrade() {
                Self::delete_selected_connections(win.upcast_ref(), &state_clone, &sidebar_clone);
            }
        });
        window.add_action(&delete_selected_action);

        // Move selected to group action
        let move_selected_action = gio::SimpleAction::new("move-selected-to-group", None);
        let window_weak = window.downgrade();
        let state_clone = state.clone();
        let sidebar_clone = sidebar.clone();
        move_selected_action.connect_activate(move |_, _| {
            if let Some(win) = window_weak.upgrade() {
                Self::show_move_selected_to_group_dialog(
                    win.upcast_ref(),
                    &state_clone,
                    &sidebar_clone,
                );
            }
        });
        window.add_action(&move_selected_action);

        // Sort connections action
        let sort_action = gio::SimpleAction::new("sort-connections", None);
        let state_clone = state.clone();
        let sidebar_clone = sidebar.clone();
        sort_action.connect_activate(move |_, _| {
            Self::sort_connections(&state_clone, &sidebar_clone);
        });
        window.add_action(&sort_action);

        // Sort recent action
        let sort_recent_action = gio::SimpleAction::new("sort-recent", None);
        let state_clone = state.clone();
        let sidebar_clone = sidebar.clone();
        sort_recent_action.connect_activate(move |_, _| {
            Self::sort_recent(&state_clone, &sidebar_clone);
        });
        window.add_action(&sort_recent_action);

        // Create cluster from sidebar selection
        let cluster_from_selection_action = gio::SimpleAction::new("cluster-from-selection", None);
        let window_weak = window.downgrade();
        let state_clone = state.clone();
        let notebook_clone = terminal_notebook.clone();
        let sidebar_clone = sidebar.clone();
        let toast_clone = self.toast_overlay.clone();
        cluster_from_selection_action.connect_activate(move |_, _| {
            if let Some(win) = window_weak.upgrade() {
                let selected_ids = sidebar_clone.get_selected_ids();
                if selected_ids.is_empty() {
                    return;
                }
                clusters::show_new_cluster_dialog_with_selection(
                    win.upcast_ref(),
                    state_clone.clone(),
                    notebook_clone.clone(),
                    selected_ids,
                    toast_clone.clone(),
                );
            }
        });
        window.add_action(&cluster_from_selection_action);
    }
}
