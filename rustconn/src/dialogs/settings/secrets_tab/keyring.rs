//! System keyring helpers for storing/retrieving secret backend credentials.

/// Saves Bitwarden master password to system keyring via rustconn-core
pub(super) fn save_bw_password_to_keyring(password: &str) {
    let secret = secrecy::SecretString::from(password.to_owned());
    match crate::async_utils::with_runtime(|rt| {
        rt.block_on(rustconn_core::secret::store_master_password_in_keyring(
            &secret,
        ))
    }) {
        Ok(Ok(())) => {
            tracing::info!("Bitwarden master password saved to keyring");
        }
        Ok(Err(e)) => {
            tracing::warn!(%e, "Failed to save Bitwarden password to keyring");
        }
        Err(e) => {
            tracing::warn!(%e, "Runtime error saving Bitwarden password to keyring");
        }
    }
}

/// Loads Bitwarden master password from system keyring via rustconn-core
pub(super) fn get_bw_password_from_keyring() -> Option<secrecy::SecretString> {
    let result = crate::async_utils::with_runtime(|rt| {
        rt.block_on(rustconn_core::secret::get_master_password_from_keyring())
    });
    match result {
        Ok(Ok(Some(secret))) => {
            tracing::debug!("Bitwarden master password loaded from keyring");
            Some(secret)
        }
        Ok(Ok(None)) => {
            tracing::debug!("No Bitwarden password found in keyring");
            None
        }
        Ok(Err(e)) => {
            tracing::debug!(%e, "Failed to load Bitwarden password from keyring");
            None
        }
        Err(e) => {
            tracing::debug!(%e, "Runtime error loading Bitwarden password from keyring");
            None
        }
    }
}

/// Saves 1Password service account token to system keyring
pub(super) fn save_op_token_to_keyring(token: &str) {
    let secret = secrecy::SecretString::from(token.to_owned());
    match crate::async_utils::with_runtime(|rt| {
        rt.block_on(rustconn_core::secret::store_token_in_keyring(&secret))
    }) {
        Ok(Ok(())) => {
            tracing::info!("1Password token saved to keyring");
        }
        Ok(Err(e)) => {
            tracing::warn!(%e, "Failed to save 1Password token to keyring");
        }
        Err(e) => {
            tracing::warn!(%e, "Runtime error saving 1Password token");
        }
    }
}

/// Loads 1Password service account token from system keyring
pub(super) fn get_op_token_from_keyring() -> Option<secrecy::SecretString> {
    let result = crate::async_utils::with_runtime(|rt| {
        rt.block_on(rustconn_core::secret::get_token_from_keyring())
    });
    match result {
        Ok(Ok(Some(secret))) => {
            tracing::debug!("1Password token loaded from keyring");
            Some(secret)
        }
        Ok(Ok(None) | Err(_)) | Err(_) => None,
    }
}

/// Saves Passbolt GPG passphrase to system keyring
pub(super) fn save_pb_passphrase_to_keyring(passphrase: &str) {
    let secret = secrecy::SecretString::from(passphrase.to_owned());
    match crate::async_utils::with_runtime(|rt| {
        rt.block_on(rustconn_core::secret::store_passphrase_in_keyring(&secret))
    }) {
        Ok(Ok(())) => {
            tracing::info!("Passbolt passphrase saved to keyring");
        }
        Ok(Err(e)) => {
            tracing::warn!(%e, "Failed to save Passbolt passphrase to keyring");
        }
        Err(e) => {
            tracing::warn!(%e, "Runtime error saving Passbolt passphrase");
        }
    }
}

/// Loads Passbolt GPG passphrase from system keyring
pub(super) fn get_pb_passphrase_from_keyring() -> Option<secrecy::SecretString> {
    let result = crate::async_utils::with_runtime(|rt| {
        rt.block_on(rustconn_core::secret::get_passphrase_from_keyring())
    });
    match result {
        Ok(Ok(Some(secret))) => {
            tracing::debug!("Passbolt passphrase loaded from keyring");
            Some(secret)
        }
        Ok(Ok(None) | Err(_)) | Err(_) => None,
    }
}

/// Saves KDBX database password to system keyring
pub(super) fn save_kdbx_password_to_keyring(password: &str) {
    let secret = secrecy::SecretString::from(password.to_owned());
    match crate::async_utils::with_runtime(|rt| {
        rt.block_on(rustconn_core::secret::store_kdbx_password_in_keyring(
            &secret,
        ))
    }) {
        Ok(Ok(())) => {
            tracing::info!("KDBX password saved to keyring");
        }
        Ok(Err(e)) => {
            tracing::warn!(%e, "Failed to save KDBX password to keyring");
        }
        Err(e) => {
            tracing::warn!(%e, "Runtime error saving KDBX password");
        }
    }
}

/// Loads KDBX database password from system keyring
pub(super) fn get_kdbx_password_from_keyring() -> Option<secrecy::SecretString> {
    let result = crate::async_utils::with_runtime(|rt| {
        rt.block_on(rustconn_core::secret::get_kdbx_password_from_keyring())
    });
    match result {
        Ok(Ok(Some(secret))) => {
            tracing::debug!("KDBX password loaded from keyring");
            Some(secret)
        }
        Ok(Ok(None) | Err(_)) | Err(_) => None,
    }
}
