//! Split view module for tab-scoped split layouts
//!
//! This module provides the GUI layer implementation for the split view redesign.
//! It bridges the core data models from `rustconn-core::split` with GTK4/libadwaita
//! widgets.
//!
//! # Architecture
//!
//! The split view system is divided between two crates:
//!
//! - **`rustconn-core::split`**: Core data models (`SplitLayoutModel`, `PanelNode`, etc.)
//! - **`rustconn::split_view`**: GUI adapters and GTK widget management
//!
//! This separation ensures that business logic can be tested without GTK dependencies.
//!
//! # Module Structure
//!
//! - `adapter` - `SplitViewAdapter` bridging core models to GTK widgets
//! - `types` - GUI-specific types (`DropSource`, `ConnectionId`)
//! - `bridge` - `SplitViewBridge` providing legacy-compatible API over new system
//!
//! # Example
//!
//! ```ignore
//! use rustconn::split_view::{DropSource, ConnectionId, SplitViewAdapter};
//! use rustconn_core::split::{SessionId, SplitDirection};
//!
//! // Create a new split view adapter
//! let mut adapter = SplitViewAdapter::new();
//!
//! // Split the focused panel vertically
//! let new_panel_id = adapter.split(SplitDirection::Vertical).unwrap();
//!
//! // Create a drop source for a sidebar item
//! let connection_id = ConnectionId::new();
//! let source = DropSource::sidebar_item(connection_id);
//!
//! // Create a drop source for a root tab
//! let session_id = SessionId::new();
//! let source = DropSource::root_tab(session_id);
//! ```

mod adapter;
mod bridge;
pub mod types;

// Re-export the new adapter
pub use adapter::SplitViewAdapter;

// Re-export the bridge for legacy-compatible API (replaces SplitTerminalView)
pub use bridge::{
    SPLIT_COLOR_VALUES, SPLIT_PANE_COLORS, SessionColorMap, SharedSessions, SharedTerminals,
    SplitDirection, SplitViewBridge, create_colored_circle_icon, get_split_color_class,
    get_split_indicator_class, get_tab_color_class,
};

// Re-export GUI-specific types
pub use types::{ConnectionId, DropOutcome, DropSource, EvictionAction, SourceCleanup};

use gtk4::prelude::*;
use rustconn_core::models::WorkspaceSplitLayout;

/// Restores a saved workspace split layout onto the active window.
///
/// `WorkspaceSplitLayout` captures a single split (direction only), so this reuses
/// the working per-session split machinery by activating the window's `split-*`
/// action instead of duplicating the bridge/adapter wiring. The split is deferred
/// to the next main-loop iteration so freshly-opened session tabs are registered
/// before the active session is split.
///
/// ponytail: restores split direction only, not `split_ratio` (panes open 50/50);
/// upgrade path: expose a ratio setter on `SplitViewBridge` and apply it post-split.
pub fn apply_layout(window: &gtk4::Window, layout: &WorkspaceSplitLayout) {
    if !layout.is_split {
        return;
    }
    let action = if layout.horizontal {
        "win.split-horizontal"
    } else {
        "win.split-vertical"
    };
    let window_weak = window.downgrade();
    gtk4::glib::idle_add_local_once(move || {
        if let Some(win) = window_weak.upgrade() {
            let _ = WidgetExt::activate_action(&win, action, None);
        }
    });
}
