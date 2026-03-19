//! MOSH protocol options for the connection dialog
//!
//! UI panel for MOSH connections with SSH port, UDP port range,
//! predict mode, and server binary settings.
//! MOSH uses the `mosh` CLI client via VTE terminal.

use super::protocol_layout::ProtocolLayoutBuilder;
use super::widgets::{EntryRowBuilder, SpinRowBuilder};
use adw::prelude::*;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, DropDown, Entry, SpinButton, StringList};
use libadwaita as adw;

use crate::i18n::i18n;

/// Return type for MOSH options creation
///
/// Contains:
/// - Container box
/// - SSH Port spin button
/// - Port Range entry
/// - Predict Mode dropdown
/// - Server Binary entry
pub type MoshOptionsWidgets = (GtkBox, SpinButton, Entry, DropDown, Entry);

/// Creates the MOSH options panel using libadwaita components.
///
/// The panel has groups for SSH handshake settings and MOSH-specific options.
#[must_use]
pub fn create_mosh_options() -> MoshOptionsWidgets {
    let (container, content) = ProtocolLayoutBuilder::new().build();

    // === SSH Handshake Group ===
    let ssh_group = adw::PreferencesGroup::builder()
        .title(i18n("SSH Handshake"))
        .description(i18n("MOSH uses SSH for the initial connection handshake."))
        .build();

    let (ssh_port_row, ssh_port_spin) = SpinRowBuilder::new("SSH Port")
        .subtitle("Port for the initial SSH handshake")
        .range(1.0, 65535.0)
        .value(22.0)
        .build();
    ssh_group.add(&ssh_port_row);

    content.append(&ssh_group);

    // === MOSH Settings Group ===
    let mosh_group = adw::PreferencesGroup::builder()
        .title(i18n("MOSH Settings"))
        .description(i18n("Configure UDP port range and prediction behavior."))
        .build();

    let (port_range_row, port_range_entry) = EntryRowBuilder::new("Port Range")
        .subtitle("UDP port range for MOSH (start:end)")
        .placeholder("60000:60010")
        .build();
    mosh_group.add(&port_range_row);

    // Predict Mode dropdown
    let predict_items = [i18n("Adaptive"), i18n("Always"), i18n("Never")];
    let predict_strs: Vec<&str> = predict_items.iter().map(String::as_str).collect();
    let predict_model = StringList::new(&predict_strs);
    let predict_dropdown = DropDown::builder()
        .model(&predict_model)
        .selected(0)
        .build();
    let predict_row = adw::ActionRow::builder()
        .title(i18n("Predict Mode"))
        .subtitle(i18n("Controls speculative local echo of keystrokes"))
        .build();
    predict_row.add_suffix(&predict_dropdown);
    predict_row.set_activatable_widget(Some(&predict_dropdown));
    mosh_group.add(&predict_row);

    let (server_binary_row, server_binary_entry) = EntryRowBuilder::new("Server Binary")
        .subtitle("Path to mosh-server on the remote host (optional)")
        .placeholder("/usr/bin/mosh-server")
        .build();
    mosh_group.add(&server_binary_row);

    content.append(&mosh_group);

    (
        container,
        ssh_port_spin,
        port_range_entry,
        predict_dropdown,
        server_binary_entry,
    )
}
