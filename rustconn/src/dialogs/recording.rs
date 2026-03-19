//! Recordings dialog for managing session recordings
//!
//! Provides a GTK4/libadwaita dialog for listing, playing, renaming,
//! deleting, and importing session recordings. Follows the same
//! `ClusterListDialog` / `TemplateManagerDialog` pattern.

use crate::i18n::i18n;
use adw::prelude::*;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{
    Box as GtkBox, Button, FileDialog, Label, ListBox, ListBoxRow, Orientation, ScrolledWindow,
};
use libadwaita as adw;
use rustconn_core::session::recording::{RecordingEntry, RecordingManager, default_recordings_dir};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

// ---------------------------------------------------------------------------
// RecordingListRow
// ---------------------------------------------------------------------------

/// A single row in the recordings list.
#[allow(dead_code)] // Fields kept for GTK widget lifecycle
struct RecordingListRow {
    row: ListBoxRow,
    data_path: PathBuf,
    entry: RecordingEntry,
    name_label: Label,
    date_label: Label,
    duration_label: Label,
    size_label: Label,
    play_button: Button,
    rename_button: Button,
    export_button: Button,
    delete_button: Button,
}

// ---------------------------------------------------------------------------
// RecordingsDialog
// ---------------------------------------------------------------------------

/// Dialog for managing session recordings (list, play, rename, delete, import).
pub struct RecordingsDialog {
    window: adw::Window,
    recordings_list: ListBox,
    recording_rows: Rc<RefCell<Vec<RecordingListRow>>>,
    on_play: Rc<RefCell<Option<Box<dyn Fn(RecordingEntry)>>>>,
    on_delete: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
    on_rename: Rc<RefCell<Option<Box<dyn Fn(PathBuf, String)>>>>,
    on_import: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl RecordingsDialog {
    /// Creates a new recordings dialog.
    #[must_use]
    pub fn new(parent: Option<&gtk4::Window>) -> Self {
        let window = adw::Window::builder()
            .title(i18n("Recordings"))
            .modal(true)
            .default_width(600)
            .default_height(450)
            .build();

        if let Some(p) = parent {
            window.set_transient_for(Some(p));
        }

        window.set_size_request(400, 300);

        // Header bar with Close + Import buttons
        let header = adw::HeaderBar::new();
        header.set_show_end_title_buttons(false);
        header.set_show_start_title_buttons(false);

        let close_btn = Button::builder().label(i18n("Close")).build();
        close_btn.update_property(&[gtk4::accessible::Property::Label(&i18n("Close recordings dialog"))]);
        header.pack_start(&close_btn);

        let import_btn = Button::builder()
            .icon_name("document-open-symbolic")
            .tooltip_text(i18n("Import recording"))
            .build();
        import_btn.update_property(&[gtk4::accessible::Property::Label(&i18n("Import recording"))]);
        header.pack_end(&import_btn);

        // Close handler
        let window_clone = window.clone();
        close_btn.connect_clicked(move |_| {
            window_clone.close();
        });

        // Scrollable content
        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .build();

        let clamp = adw::Clamp::builder()
            .maximum_size(600)
            .tightening_threshold(400)
            .build();

        let recordings_list = ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(["boxed-list"])
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();

        // Placeholder when list is empty
        let placeholder = adw::StatusPage::builder()
            .icon_name("media-tape-symbolic")
            .title(i18n("No Recordings"))
            .description(i18n(
                "Session recordings will appear here. Use the context menu to start recording a session.",
            ))
            .build();
        recordings_list.set_placeholder(Some(&placeholder));

        clamp.set_child(Some(&recordings_list));
        scrolled.set_child(Some(&clamp));

        let toolbar_view = adw::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&scrolled));
        window.set_content(Some(&toolbar_view));

        // Callbacks
        let on_play: Rc<RefCell<Option<Box<dyn Fn(RecordingEntry)>>>> = Rc::new(RefCell::new(None));
        let on_delete: Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>> = Rc::new(RefCell::new(None));
        let on_rename: Rc<RefCell<Option<Box<dyn Fn(PathBuf, String)>>>> =
            Rc::new(RefCell::new(None));
        let on_import: Rc<RefCell<Option<Box<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let recording_rows: Rc<RefCell<Vec<RecordingListRow>>> = Rc::new(RefCell::new(Vec::new()));

        // Import button handler
        let window_weak = window.downgrade();
        let on_import_clone = on_import.clone();
        import_btn.connect_clicked(move |_| {
            let Some(win) = window_weak.upgrade() else {
                return;
            };
            Self::handle_import(&win);
            if let Some(ref cb) = *on_import_clone.borrow() {
                cb();
            }
        });

        let dialog = Self {
            window,
            recordings_list,
            recording_rows,
            on_play,
            on_delete,
            on_rename,
            on_import,
        };

        dialog.refresh_list();
        dialog
    }

    /// Refreshes the recordings list from disk via `RecordingManager`.
    pub fn refresh_list(&self) {
        // Clear existing rows
        while let Some(row) = self.recordings_list.row_at_index(0) {
            self.recordings_list.remove(&row);
        }
        self.recording_rows.borrow_mut().clear();

        let Some(dir) = default_recordings_dir() else {
            return;
        };

        let mgr = RecordingManager::new(dir);
        let entries = match mgr.list() {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("Failed to list recordings: {e}");
                return;
            }
        };

        for entry in entries {
            let list_row = self.create_recording_row(&entry);
            self.recordings_list.append(&list_row.row);
            self.recording_rows.borrow_mut().push(list_row);
        }
    }

    /// Shows the dialog.
    pub fn present(&self) {
        self.window.present();
    }

    /// Returns a reference to the underlying window.
    #[must_use]
    pub const fn window(&self) -> &adw::Window {
        &self.window
    }

    /// Sets the callback invoked when the user clicks Play on a recording.
    pub fn set_on_play<F: Fn(RecordingEntry) + 'static>(&self, cb: F) {
        *self.on_play.borrow_mut() = Some(Box::new(cb));
    }

    /// Sets the callback invoked when the user deletes a recording.
    pub fn set_on_delete<F: Fn(PathBuf) + 'static>(&self, cb: F) {
        *self.on_delete.borrow_mut() = Some(Box::new(cb));
    }

    /// Sets the callback invoked when the user renames a recording.
    pub fn set_on_rename<F: Fn(PathBuf, String) + 'static>(&self, cb: F) {
        *self.on_rename.borrow_mut() = Some(Box::new(cb));
    }

    /// Sets the callback invoked when the user clicks Import.
    pub fn set_on_import<F: Fn() + 'static>(&self, cb: F) {
        *self.on_import.borrow_mut() = Some(Box::new(cb));
    }

    /// Creates a list row widget for a single recording entry.
    fn create_recording_row(&self, entry: &RecordingEntry) -> RecordingListRow {
        let hbox = GtkBox::new(Orientation::Horizontal, 8);
        hbox.set_margin_top(8);
        hbox.set_margin_bottom(8);
        hbox.set_margin_start(12);
        hbox.set_margin_end(12);

        // Info column
        let info_box = GtkBox::new(Orientation::Vertical, 2);
        info_box.set_hexpand(true);

        let display = entry
            .metadata
            .display_name
            .as_deref()
            .unwrap_or(&entry.metadata.connection_name);
        let name_label = Label::builder()
            .label(display)
            .halign(gtk4::Align::Start)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .css_classes(["heading"])
            .build();
        info_box.append(&name_label);

        // Date + duration + size on second line
        let date_str = entry
            .metadata
            .created_at
            .format("%Y-%m-%d %H:%M")
            .to_string();
        let date_label = Label::builder()
            .label(&date_str)
            .halign(gtk4::Align::Start)
            .css_classes(["dim-label", "caption"])
            .build();

        let duration_label = Label::builder()
            .label(&format_duration(entry.metadata.duration_secs))
            .halign(gtk4::Align::Start)
            .css_classes(["dim-label", "caption"])
            .build();

        let size_label = Label::builder()
            .label(&format_size(entry.metadata.total_size_bytes))
            .halign(gtk4::Align::Start)
            .css_classes(["dim-label", "caption"])
            .build();

        let details_box = GtkBox::new(Orientation::Horizontal, 8);
        details_box.append(&date_label);
        details_box.append(&duration_label);
        details_box.append(&size_label);
        info_box.append(&details_box);

        hbox.append(&info_box);

        // Action buttons
        let play_label = format!("{} {}", i18n("Play recording"), display);
        let play_button = Button::builder()
            .icon_name("media-playback-start-symbolic")
            .tooltip_text(i18n("Play recording"))
            .css_classes(["flat"])
            .valign(gtk4::Align::Center)
            .build();
        play_button.update_property(&[gtk4::accessible::Property::Label(&play_label)]);

        let rename_label = format!("{} {}", i18n("Rename recording"), display);
        let rename_button = Button::builder()
            .icon_name("document-edit-symbolic")
            .tooltip_text(i18n("Rename recording"))
            .css_classes(["flat"])
            .valign(gtk4::Align::Center)
            .build();
        rename_button.update_property(&[gtk4::accessible::Property::Label(&rename_label)]);

        let delete_label = format!("{} {}", i18n("Delete recording"), display);
        let delete_button = Button::builder()
            .icon_name("user-trash-symbolic")
            .tooltip_text(i18n("Delete recording"))
            .css_classes(["flat", "destructive-action"])
            .valign(gtk4::Align::Center)
            .build();
        delete_button.update_property(&[gtk4::accessible::Property::Label(&delete_label)]);

        let export_label = format!("{} {}", i18n("Export recording"), display);
        let export_button = Button::builder()
            .icon_name("document-save-symbolic")
            .tooltip_text(i18n("Export recording"))
            .css_classes(["flat"])
            .valign(gtk4::Align::Center)
            .build();
        export_button.update_property(&[gtk4::accessible::Property::Label(&export_label)]);

        hbox.append(&play_button);
        hbox.append(&rename_button);
        hbox.append(&export_button);
        hbox.append(&delete_button);

        let row_label = format!(
            "{}, {}, {}, {}",
            display, date_str, format_duration(entry.metadata.duration_secs), format_size(entry.metadata.total_size_bytes)
        );
        let row = ListBoxRow::builder()
            .child(&hbox)
            .selectable(false)
            .focusable(true)
            .build();
        row.update_property(&[gtk4::accessible::Property::Label(&row_label)]);

        // Wire up Play
        let entry_clone = entry.clone();
        let on_play = self.on_play.clone();
        play_button.connect_clicked(move |_| {
            if let Some(ref cb) = *on_play.borrow() {
                cb(entry_clone.clone());
            }
        });

        // Wire up Rename
        let data_path = entry.data_path.clone();
        let window_weak = self.window.downgrade();
        let on_rename = self.on_rename.clone();
        let name_label_clone = name_label.clone();
        let current_name = display.to_string();
        rename_button.connect_clicked(move |_| {
            let Some(win) = window_weak.upgrade() else {
                return;
            };
            Self::handle_rename(
                &win,
                &data_path,
                &current_name,
                &on_rename,
                &name_label_clone,
            );
        });

        // Wire up Delete
        let data_path_del = entry.data_path.clone();
        let window_weak_del = self.window.downgrade();
        let on_delete = self.on_delete.clone();
        let recordings_list_ref = self.recordings_list.clone();
        let recording_rows_ref = self.recording_rows.clone();
        let row_weak = row.downgrade();
        let entry_name = display.to_string();
        delete_button.connect_clicked(move |_| {
            let Some(win) = window_weak_del.upgrade() else {
                return;
            };
            Self::handle_delete(
                &win,
                &data_path_del,
                &entry_name,
                &on_delete,
                &recordings_list_ref,
                &recording_rows_ref,
                &row_weak,
            );
        });

        // Wire up Export
        let data_path_exp = entry.data_path.clone();
        let window_weak_exp = self.window.downgrade();
        export_button.connect_clicked(move |_| {
            let Some(win) = window_weak_exp.upgrade() else {
                return;
            };
            Self::handle_export(&win, &data_path_exp);
        });

        RecordingListRow {
            row,
            data_path: entry.data_path.clone(),
            entry: entry.clone(),
            name_label,
            date_label,
            duration_label,
            size_label,
            play_button,
            rename_button,
            export_button,
            delete_button,
        }
    }

    // -----------------------------------------------------------------------
    // Rename handler
    // -----------------------------------------------------------------------

    /// Shows a rename dialog and updates the metadata sidecar on disk.
    fn handle_rename(
        win: &adw::Window,
        data_path: &std::path::Path,
        current_name: &str,
        on_rename: &Rc<RefCell<Option<Box<dyn Fn(PathBuf, String)>>>>,
        name_label: &Label,
    ) {
        let alert = adw::AlertDialog::builder()
            .heading(i18n("Rename Recording"))
            .body(i18n("Enter a new display name for this recording."))
            .build();

        alert.add_response("cancel", &i18n("Cancel"));
        alert.add_response("rename", &i18n("Rename"));
        alert.set_response_appearance("rename", adw::ResponseAppearance::Suggested);
        alert.set_default_response(Some("rename"));
        alert.set_close_response("cancel");

        // Add an entry row for the new name
        let entry = adw::EntryRow::builder()
            .title(i18n("Display Name"))
            .text(current_name)
            .show_apply_button(false)
            .build();
        alert.set_extra_child(Some(&entry));

        let data_path = data_path.to_path_buf();
        let on_rename = on_rename.clone();
        let name_label = name_label.clone();
        alert.connect_response(None, move |_, response| {
            if response != "rename" {
                return;
            }
            let new_name = entry.text().trim().to_string();
            if new_name.is_empty() {
                return;
            }

            // Persist to disk
            let Some(dir) = default_recordings_dir() else {
                return;
            };
            let mgr = RecordingManager::new(dir);
            if let Err(e) = mgr.rename(&data_path, &new_name) {
                tracing::warn!("Failed to rename recording: {e}");
                return;
            }

            // Update the label in-place
            name_label.set_label(&new_name);

            if let Some(ref cb) = *on_rename.borrow() {
                cb(data_path.clone(), new_name);
            }
        });

        alert.present(Some(win));
    }

    // -----------------------------------------------------------------------
    // Delete handler
    // -----------------------------------------------------------------------

    /// Shows a confirmation dialog and deletes the recording from disk.
    #[allow(clippy::too_many_arguments)]
    fn handle_delete(
        win: &adw::Window,
        data_path: &std::path::Path,
        entry_name: &str,
        on_delete: &Rc<RefCell<Option<Box<dyn Fn(PathBuf)>>>>,
        recordings_list: &ListBox,
        recording_rows: &Rc<RefCell<Vec<RecordingListRow>>>,
        row_weak: &glib::WeakRef<ListBoxRow>,
    ) {
        let alert = adw::AlertDialog::builder()
            .heading(i18n("Delete Recording?"))
            .body(format!(
                "{}\n\n{}",
                entry_name,
                i18n("This will permanently remove the recording files from disk.")
            ))
            .build();

        alert.add_response("cancel", &i18n("Cancel"));
        alert.add_response("delete", &i18n("Delete"));
        alert.set_response_appearance("delete", adw::ResponseAppearance::Destructive);
        alert.set_default_response(Some("cancel"));
        alert.set_close_response("cancel");

        let data_path = data_path.to_path_buf();
        let on_delete = on_delete.clone();
        let recordings_list = recordings_list.clone();
        let recording_rows = recording_rows.clone();
        let row_weak = row_weak.clone();
        alert.connect_response(None, move |_, response| {
            if response != "delete" {
                return;
            }

            // Delete from disk
            let Some(dir) = default_recordings_dir() else {
                return;
            };
            let mgr = RecordingManager::new(dir);
            if let Err(e) = mgr.delete(&data_path) {
                tracing::warn!("Failed to delete recording: {e}");
                return;
            }

            // Remove the row from the list
            if let Some(r) = row_weak.upgrade() {
                recordings_list.remove(&r);
            }
            recording_rows
                .borrow_mut()
                .retain(|rr| rr.data_path != data_path);

            if let Some(ref cb) = *on_delete.borrow() {
                cb(data_path.clone());
            }
        });

        alert.present(Some(win));
    }

    // -----------------------------------------------------------------------
    // Import handler
    // -----------------------------------------------------------------------

    /// Opens file chooser dialogs for data + timing files, validates, and
    /// imports the recording.
    fn handle_export(win: &adw::Window, data_path: &std::path::Path) {
        let dialog = FileDialog::builder()
            .title(i18n("Export Recording"))
            .modal(true)
            .build();

        let win_clone = win.clone();
        let data_path = data_path.to_path_buf();

        dialog.select_folder(Some(win), gtk4::gio::Cancellable::NONE, move |result| {
            let Ok(folder) = result else {
                return; // User cancelled
            };
            let Some(dest_dir) = folder.path() else {
                return;
            };

            let Some(rec_dir) = default_recordings_dir() else {
                Self::show_error(&win_clone, &i18n("Cannot determine recordings directory."));
                return;
            };

            let mgr = RecordingManager::new(rec_dir);
            match mgr.export(&data_path, &dest_dir) {
                Ok(_) => {
                    crate::toast::show_toast_on_window(
                        &win_clone,
                        &i18n("Recording exported successfully"),
                        crate::toast::ToastType::Info,
                    );
                }
                Err(e) => {
                    let msg = format!("{}: {e}", i18n("Export failed"));
                    Self::show_error(&win_clone, &msg);
                }
            }
        });
    }

    fn handle_import(win: &adw::Window) {
        // Step 1: Choose the data file
        let data_filter = gtk4::FileFilter::new();
        data_filter.set_name(Some(&i18n("Data files")));
        data_filter.add_pattern("*.data");
        data_filter.add_pattern("*");

        let filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
        filters.append(&data_filter);

        let data_dialog = FileDialog::builder()
            .title(i18n("Select Data File"))
            .modal(true)
            .filters(&filters)
            .build();

        let win_clone = win.clone();

        data_dialog.open(Some(win), gtk4::gio::Cancellable::NONE, move |result| {
            let Ok(data_file) = result else {
                return; // User cancelled
            };
            let Some(data_path) = data_file.path() else {
                return;
            };

            // Step 2: Choose the timing file
            let timing_filter = gtk4::FileFilter::new();
            timing_filter.set_name(Some(&i18n("Timing files")));
            timing_filter.add_pattern("*.timing");
            timing_filter.add_pattern("*");

            let timing_filters = gtk4::gio::ListStore::new::<gtk4::FileFilter>();
            timing_filters.append(&timing_filter);

            let timing_dialog = FileDialog::builder()
                .title(i18n("Select Timing File"))
                .modal(true)
                .filters(&timing_filters)
                .build();

            let win_inner = win_clone.clone();

            timing_dialog.open(
                Some(&win_clone),
                gtk4::gio::Cancellable::NONE,
                move |result| {
                    let Ok(timing_file) = result else {
                        return;
                    };
                    let Some(timing_path) = timing_file.path() else {
                        return;
                    };

                    Self::do_import(&win_inner, &data_path, &timing_path);
                },
            );
        });
    }

    /// Performs the actual import after both files have been selected.
    fn do_import(win: &adw::Window, data_path: &std::path::Path, timing_path: &std::path::Path) {
        let Some(dir) = default_recordings_dir() else {
            Self::show_error(win, &i18n("Cannot determine recordings directory."));
            return;
        };

        let mgr = RecordingManager::new(dir);
        match mgr.import(data_path, timing_path) {
            Ok(_entry) => {
                crate::toast::show_toast_on_window(
                    win,
                    &i18n("Recording imported successfully"),
                    crate::toast::ToastType::Info,
                );
            }
            Err(e) => {
                let msg = format!("{}: {e}", i18n("Import failed"));
                Self::show_error(win, &msg);
            }
        }
    }

    /// Shows an error alert dialog.
    fn show_error(win: &adw::Window, message: &str) {
        let alert = adw::AlertDialog::builder()
            .heading(i18n("Error"))
            .body(message)
            .build();
        alert.add_response("ok", &i18n("OK"));
        alert.set_default_response(Some("ok"));
        alert.set_close_response("ok");
        alert.present(Some(win));
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

/// Formats a duration in seconds to a human-readable string (e.g. "5m 23s").
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn format_duration(secs: f64) -> String {
    let total = secs as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;

    if hours > 0 {
        format!("{hours}h {minutes:02}m {seconds:02}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

/// Formats a byte count to a human-readable size string.
#[allow(clippy::cast_precision_loss)]
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;

    if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}
