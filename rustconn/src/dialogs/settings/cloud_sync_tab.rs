//! Cloud Sync preferences page for the Settings dialog.
//!
//! Implements the `AdwPreferencesPage` "Cloud Sync" with four groups:
//! - Setup: sync directory and device name
//! - Synced Groups: list of groups with sync mode
//! - Available in Cloud: unimported `.rcn` files
//! - Simple Sync: toggle for bidirectional sync

use adw::prelude::*;
use gtk4::prelude::*;
use libadwaita as adw;

use crate::i18n::i18n;

/// Widgets from the Cloud Sync preferences page that need to be accessed
/// for loading/saving settings.
pub struct CloudSyncPageWidgets {
    /// The preferences page itself.
    pub page: adw::PreferencesPage,
    /// Sync directory entry row.
    pub sync_dir_row: adw::EntryRow,
    /// Device name entry row.
    pub device_name_row: adw::EntryRow,
    /// Container for synced group rows (dynamically populated).
    pub synced_groups_group: adw::PreferencesGroup,
    /// Container for available cloud file rows (dynamically populated).
    pub available_files_group: adw::PreferencesGroup,
    /// Simple Sync toggle.
    pub simple_sync_row: adw::SwitchRow,
}

/// Creates the Cloud Sync `AdwPreferencesPage`.
///
/// Returns the page and its interactive widgets for later data binding.
#[must_use]
pub fn create_cloud_sync_page() -> CloudSyncPageWidgets {
    let page = adw::PreferencesPage::builder()
        .title(i18n("Cloud Sync"))
        .icon_name("emblem-synchronizing-symbolic")
        .build();

    // --- Setup group ---
    let setup_group = adw::PreferencesGroup::builder()
        .title(i18n("Setup"))
        .build();

    let sync_dir_row = adw::EntryRow::builder()
        .title(i18n("Sync Directory"))
        .show_apply_button(true)
        .build();

    // File chooser suffix button for sync directory
    let dir_button = gtk4::Button::from_icon_name("folder-open-symbolic");
    dir_button.set_valign(gtk4::Align::Center);
    dir_button.set_tooltip_text(Some(&i18n("Choose sync directory")));
    dir_button.update_property(&[gtk4::accessible::Property::Label(&i18n(
        "Choose sync directory",
    ))]);
    sync_dir_row.add_suffix(&dir_button);

    // Wire up file chooser button
    let sync_dir_row_clone = sync_dir_row.clone();
    dir_button.connect_clicked(move |btn| {
        let Some(root) = btn.root() else { return };
        let Some(win) = root.downcast_ref::<gtk4::Window>() else {
            return;
        };
        let file_dialog = gtk4::FileDialog::builder()
            .title(i18n("Select Sync Directory"))
            .build();
        let row_clone = sync_dir_row_clone.clone();
        file_dialog.select_folder(Some(win), None::<&gtk4::gio::Cancellable>, move |result| {
            if let Ok(file) = result
                && let Some(path) = file.path()
            {
                row_clone.set_text(&path.to_string_lossy());
            }
        });
    });

    setup_group.add(&sync_dir_row);

    let device_name_row = adw::EntryRow::builder()
        .title(i18n("Device Name"))
        .show_apply_button(true)
        .build();
    setup_group.add(&device_name_row);

    page.add(&setup_group);

    // --- Synced Groups group ---
    let synced_groups_group = adw::PreferencesGroup::builder()
        .title(i18n("Synced Groups"))
        .build();
    page.add(&synced_groups_group);

    // --- Available in Cloud group ---
    let available_files_group = adw::PreferencesGroup::builder()
        .title(i18n("Available in Cloud"))
        .description(i18n("Files in sync directory not yet imported"))
        .build();
    page.add(&available_files_group);

    // --- Simple Sync group ---
    let simple_sync_group = adw::PreferencesGroup::builder()
        .title(i18n("Simple Sync"))
        .build();

    let simple_sync_row = adw::SwitchRow::builder()
        .title(i18n("Sync everything between your devices"))
        .build();
    simple_sync_group.add(&simple_sync_row);

    page.add(&simple_sync_group);

    CloudSyncPageWidgets {
        page,
        sync_dir_row,
        device_name_row,
        synced_groups_group,
        available_files_group,
        simple_sync_row,
    }
}

/// Adds a synced group row to the "Synced Groups" preferences group.
///
/// Displays the group name with a subtitle indicating its sync mode
/// (e.g., "Master · synced" or "Import · synced").
pub fn add_synced_group_row(group: &adw::PreferencesGroup, name: &str, sync_mode: &str) {
    let subtitle = match sync_mode {
        "master" => i18n("Master · synced"),
        "import" => i18n("Import · synced"),
        _ => i18n("Sync error"),
    };

    let row = adw::ActionRow::builder()
        .title(name)
        .subtitle(&subtitle)
        .build();
    group.add(&row);
}

/// Adds an available cloud file row with an "Import" suffix button.
///
/// The `on_import` callback is invoked when the user clicks "Import".
#[allow(dead_code)] // Will be used when import-from-settings is wired up
pub fn add_available_file_row(
    group: &adw::PreferencesGroup,
    filename: &str,
    on_import: impl Fn(&str) + 'static,
) {
    let row = adw::ActionRow::builder().title(filename).build();

    let import_btn = gtk4::Button::builder()
        .label(i18n("Import"))
        .valign(gtk4::Align::Center)
        .css_classes(["suggested-action"])
        .build();

    let filename_owned = filename.to_owned();
    import_btn.connect_clicked(move |_| {
        on_import(&filename_owned);
    });

    row.add_suffix(&import_btn);
    group.add(&row);
}
