//! Ad-hoc broadcast controller for sending input to multiple terminals.
//!
//! This is separate from the cluster broadcast mechanism — it allows
//! the user to select arbitrary open terminals and forward keystrokes
//! to all of them simultaneously without creating a persistent cluster.

use std::collections::HashSet;

use uuid::Uuid;

/// Manages ad-hoc broadcast mode where keystrokes are sent to multiple
/// selected terminals simultaneously.
pub struct BroadcastController {
    /// Whether broadcast mode is currently active.
    active: bool,
    /// Session IDs of terminals selected for broadcast.
    selected_terminals: HashSet<Uuid>,
}

impl Default for BroadcastController {
    fn default() -> Self {
        Self::new()
    }
}

impl BroadcastController {
    /// Creates a new inactive broadcast controller.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: false,
            selected_terminals: HashSet::new(),
        }
    }

    /// Activates broadcast mode.
    pub fn activate(&mut self) {
        self.active = true;
    }

    /// Deactivates broadcast mode and clears all selected terminals.
    pub fn deactivate(&mut self) {
        self.active = false;
        self.selected_terminals.clear();
    }

    /// Returns whether broadcast mode is currently active.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Toggles a terminal's selection state for broadcast.
    ///
    /// If the terminal is already selected, it is removed.
    /// If not selected, it is added.
    pub fn toggle_terminal(&mut self, session_id: Uuid) {
        if !self.selected_terminals.remove(&session_id) {
            self.selected_terminals.insert(session_id);
        }
    }

    /// Returns whether a terminal is selected for broadcast.
    #[must_use]
    pub fn is_selected(&self, session_id: &Uuid) -> bool {
        self.selected_terminals.contains(session_id)
    }

    /// Removes a terminal from the broadcast selection.
    ///
    /// Used when a terminal tab is closed while broadcast is active.
    pub fn remove_terminal(&mut self, session_id: &Uuid) {
        self.selected_terminals.remove(session_id);
    }

    /// Returns the session IDs that should receive broadcast input.
    ///
    /// When broadcast is active and terminals are selected, returns
    /// the selected terminal IDs. Otherwise returns an empty slice
    /// (caller should fall back to the focused terminal).
    #[must_use]
    pub fn broadcast_targets(&self) -> Vec<Uuid> {
        if self.active && !self.selected_terminals.is_empty() {
            self.selected_terminals.iter().copied().collect()
        } else {
            Vec::new()
        }
    }

    /// Broadcasts input text to all selected terminals.
    ///
    /// Calls the provided `feed` closure for each selected terminal.
    /// Returns the number of terminals that received the input.
    pub fn broadcast_input<F>(&self, input: &str, mut feed: F) -> usize
    where
        F: FnMut(Uuid, &str),
    {
        if !self.active || self.selected_terminals.is_empty() {
            return 0;
        }
        let mut count = 0;
        for &session_id in &self.selected_terminals {
            feed(session_id, input);
            count += 1;
        }
        count
    }

    /// Returns the number of currently selected terminals.
    #[must_use]
    pub fn selected_count(&self) -> usize {
        self.selected_terminals.len()
    }
}
