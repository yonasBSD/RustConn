//! VNC protocol options for the connection dialog
//!
//! This module provides the VNC-specific UI components including:
//! - Client mode selection (Embedded/External)
//! - Performance mode (Quality/Balanced/Speed)
//! - Encoding preferences
//! - Compression and quality settings
//! - View-only mode, scaling, clipboard sharing

// These functions are prepared for future refactoring when dialog.rs is further modularized
#![allow(dead_code)]

use super::protocol_layout::ProtocolLayoutBuilder;
use super::widgets::{CheckboxRowBuilder, DropdownRowBuilder, EntryRowBuilder, SpinRowBuilder};
use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, CheckButton, DropDown, Entry, SpinButton};
use libadwaita as adw;
use rustconn_core::models::{ScaleOverride, VncClientMode, VncPerformanceMode};

use crate::i18n::i18n;

/// Return type for VNC options creation
#[allow(clippy::type_complexity)]
pub type VncOptionsWidgets = (
    GtkBox,
    DropDown,    // client_mode_dropdown
    DropDown,    // performance_mode_dropdown
    DropDown,    // encoding_dropdown
    SpinButton,  // compression_spin
    SpinButton,  // quality_spin
    CheckButton, // view_only_check
    CheckButton, // scaling_check
    CheckButton, // clipboard_check
    CheckButton, // show_local_cursor_check
    DropDown,    // scale_override_dropdown
    Entry,       // custom_args_entry
);

/// Creates the VNC options panel using libadwaita components following GNOME HIG.
#[must_use]
pub fn create_vnc_options() -> VncOptionsWidgets {
    let (container, content) = ProtocolLayoutBuilder::new().build();

    // === Display Group ===
    let (
        display_group,
        client_mode_dropdown,
        performance_mode_dropdown,
        encoding_dropdown,
        scale_override_dropdown,
    ) = create_display_group();
    content.append(&display_group);

    // === Quality Group ===
    let (quality_group, compression_spin, quality_spin) = create_quality_group();
    content.append(&quality_group);

    // VNC-2: Sync compression/quality with Performance Mode changes
    let compression_clone = compression_spin.clone();
    let quality_clone = quality_spin.clone();
    performance_mode_dropdown.connect_selected_notify(move |dropdown| {
        let (comp, qual) = match dropdown.selected() {
            0 => (0.0, 9.0), // Quality
            2 => (9.0, 0.0), // Speed
            _ => (5.0, 5.0), // Balanced
        };
        compression_clone.set_value(comp);
        quality_clone.set_value(qual);
    });

    // === Features Group ===
    let (features_group, view_only_check, scaling_check, clipboard_check, show_local_cursor_check) =
        create_features_group();
    content.append(&features_group);

    // === Advanced Group ===
    let (advanced_group, custom_args_entry) = create_advanced_group();
    content.append(&advanced_group);

    (
        container,
        client_mode_dropdown,
        performance_mode_dropdown,
        encoding_dropdown,
        compression_spin,
        quality_spin,
        view_only_check,
        scaling_check,
        clipboard_check,
        show_local_cursor_check,
        scale_override_dropdown,
        custom_args_entry,
    )
}

/// Creates the Display preferences group
fn create_display_group() -> (
    adw::PreferencesGroup,
    DropDown,
    DropDown,
    DropDown,
    DropDown,
) {
    let display_group = adw::PreferencesGroup::builder()
        .title(i18n("Display"))
        .build();

    // Client mode dropdown
    let client_mode_items: Vec<String> = vec![
        i18n(VncClientMode::Embedded.display_name()),
        i18n(VncClientMode::External.display_name()),
    ];
    let client_mode_strs: Vec<&str> = client_mode_items.iter().map(String::as_str).collect();
    let (client_mode_row, client_mode_dropdown) = DropdownRowBuilder::new("Client Mode")
        .subtitle("Embedded renders in tab, External opens separate window")
        .items(&client_mode_strs)
        .build();
    display_group.add(&client_mode_row);

    // Performance mode dropdown
    let perf_items: Vec<String> = vec![
        i18n(VncPerformanceMode::Quality.display_name()),
        i18n(VncPerformanceMode::Balanced.display_name()),
        i18n(VncPerformanceMode::Speed.display_name()),
    ];
    let perf_strs: Vec<&str> = perf_items.iter().map(String::as_str).collect();
    let (perf_row, performance_mode_dropdown) = DropdownRowBuilder::new("Performance Mode")
        .subtitle("Quality/speed tradeoff for image rendering")
        .items(&perf_strs)
        .selected(1) // Default to Balanced
        .build();
    display_group.add(&perf_row);

    // Scale override dropdown (for embedded mode)
    let scale_items: Vec<String> = ScaleOverride::all()
        .iter()
        .map(|s| i18n(s.display_name()))
        .collect();
    let scale_strs: Vec<&str> = scale_items.iter().map(String::as_str).collect();
    let (scale_row, scale_override_dropdown) = DropdownRowBuilder::new("Display Scale")
        .subtitle("Override HiDPI scaling for embedded viewer")
        .items(&scale_strs)
        .build();
    display_group.add(&scale_row);

    // VNC-1: Encoding dropdown instead of free text entry
    let (encoding_row, encoding_dropdown) = DropdownRowBuilder::new("Encoding")
        .subtitle("Preferred encoding method (overrides Performance Mode)")
        .items(&["Auto", "Tight", "ZRLE", "Hextile", "Raw", "CopyRect"])
        .build();
    display_group.add(&encoding_row);

    // Toggle scale row visibility based on client mode
    let scale_row_clone = scale_row.clone();
    client_mode_dropdown.connect_selected_notify(move |dropdown| {
        let is_embedded = dropdown.selected() == 0;
        scale_row_clone.set_visible(is_embedded);
    });

    (
        display_group,
        client_mode_dropdown,
        performance_mode_dropdown,
        encoding_dropdown,
        scale_override_dropdown,
    )
}

/// Creates the Quality preferences group
fn create_quality_group() -> (adw::PreferencesGroup, SpinButton, SpinButton) {
    let quality_group = adw::PreferencesGroup::builder()
        .title(i18n("Quality"))
        .build();

    // Compression
    let (compression_row, compression_spin) = SpinRowBuilder::new("Compression")
        .subtitle("0 (none) to 9 (maximum)")
        .range(0.0, 9.0)
        .value(6.0)
        .build();
    quality_group.add(&compression_row);

    // Quality
    let (quality_row, quality_spin) = SpinRowBuilder::new("Quality")
        .subtitle("0 (lowest) to 9 (highest)")
        .range(0.0, 9.0)
        .value(6.0)
        .build();
    quality_group.add(&quality_row);

    (quality_group, compression_spin, quality_spin)
}

/// Creates the Features preferences group
fn create_features_group() -> (
    adw::PreferencesGroup,
    CheckButton,
    CheckButton,
    CheckButton,
    CheckButton,
) {
    let features_group = adw::PreferencesGroup::builder()
        .title(i18n("Features"))
        .build();

    // View-only mode
    let (view_only_row, view_only_check) = CheckboxRowBuilder::new("View-Only Mode")
        .subtitle("Disable keyboard and mouse input")
        .build();
    features_group.add(&view_only_row);

    // Scaling
    let (scaling_row, scaling_check) = CheckboxRowBuilder::new("Scale Display")
        .subtitle("Fit remote desktop to window size")
        .active(true)
        .build();
    features_group.add(&scaling_row);

    // Clipboard sharing
    let (clipboard_row, clipboard_check) = CheckboxRowBuilder::new("Clipboard Sharing")
        .subtitle("Synchronize clipboard with remote")
        .active(true)
        .build();
    features_group.add(&clipboard_row);

    // VNC-3: Password info row
    let password_info_row = adw::ActionRow::builder()
        .title(i18n("Authentication"))
        .subtitle(i18n("VNC uses the connection password for authentication"))
        .activatable(false)
        .build();
    password_info_row.add_prefix(&gtk4::Image::from_icon_name("dialog-information-symbolic"));
    features_group.add(&password_info_row);

    // Show local cursor
    let (show_cursor_row, show_local_cursor_check) = CheckboxRowBuilder::new("Show Local Cursor")
        .subtitle("Hide to avoid double cursor in embedded mode")
        .active(true)
        .build();
    features_group.add(&show_cursor_row);

    (
        features_group,
        view_only_check,
        scaling_check,
        clipboard_check,
        show_local_cursor_check,
    )
}

/// Creates the Advanced preferences group
fn create_advanced_group() -> (adw::PreferencesGroup, Entry) {
    let advanced_group = adw::PreferencesGroup::builder()
        .title(i18n("Advanced"))
        .build();

    let (args_row, custom_args_entry) = EntryRowBuilder::new("Custom Arguments")
        .subtitle("Extra command-line options for vncviewer")
        .placeholder("Additional arguments for external client")
        .build();
    advanced_group.add(&args_row);

    (advanced_group, custom_args_entry)
}
