//! Backend missing dialog for Cloud Sync credential resolution.
//!
//! Shown when a connection's password source references a secret backend
//! (KeePass, Bitwarden, etc.) that is not configured on this device.
//!
//! GNOME HIG: `AdwAlertDialog` with two response buttons.

use adw::prelude::*;
use libadwaita as adw;

use crate::i18n::i18n;

/// Response from the backend missing dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendMissingResponse {
    /// User chose to enter the password manually (one-time).
    EnterManually,
    /// User chose to open settings to configure the backend.
    OpenSettings,
}

/// Shows the "Secret Backend Not Configured" dialog.
///
/// Presents an `AdwAlertDialog` with:
/// - heading: "Secret Backend Not Configured"
/// - body: explanation that a secret vault is required
/// - responses: "Enter Password Manually" / "Open Settings"
///
/// # Arguments
/// * `parent` — parent widget for the dialog
/// * `callback` — called with the user's response
pub fn show_backend_missing_dialog<F>(parent: &impl IsA<gtk4::Widget>, callback: F)
where
    F: Fn(BackendMissingResponse) + 'static,
{
    let heading = i18n("Secret Backend Not Configured");
    let body =
        i18n("This connection stores credentials in a secret vault, but no backend is set up yet.");

    let dialog = adw::AlertDialog::new(Some(&heading), Some(&body));

    dialog.add_response("manual", &i18n("Enter Password Manually"));
    dialog.add_response("settings", &i18n("Open Settings"));
    dialog.set_default_response(Some("manual"));
    dialog.set_close_response("manual");
    dialog.set_response_appearance("settings", adw::ResponseAppearance::Suggested);

    dialog.connect_response(None, move |_, response| {
        if response == "settings" {
            callback(BackendMissingResponse::OpenSettings);
        } else {
            callback(BackendMissingResponse::EnterManually);
        }
    });

    dialog.present(Some(parent));
}
