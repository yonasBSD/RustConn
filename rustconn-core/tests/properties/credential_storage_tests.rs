//! Property tests for the 3-state credential storage migration
//!
//! Feature: UX-7b — `SecretSettings` exposes `*_storage()` /
//! `set_*_storage()` helpers built on top of legacy
//! `*_password_encrypted` + `*_save_to_keyring` pairs. Old configs must
//! continue to round-trip through the new API.

use proptest::prelude::*;
use rustconn_core::config::SecretSettings;
use rustconn_core::secret::CredentialStorage;

/// Reduces a legacy `(encrypted_present, save_to_keyring)` pair into a
/// canonical [`CredentialStorage`] using the same rule the production
/// helpers apply. Centralised here so the property tests document the
/// expected mapping table.
fn expected_storage(encrypted_present: bool, save_to_keyring: bool) -> CredentialStorage {
    if save_to_keyring {
        CredentialStorage::SystemKeyring
    } else if encrypted_present {
        CredentialStorage::EncryptedFile
    } else {
        CredentialStorage::None
    }
}

// Property: `from_legacy` is total and matches the canonical mapping.
proptest! {
    #[test]
    fn from_legacy_matches_canonical_mapping(
        encrypted in any::<bool>(),
        keyring in any::<bool>(),
    ) {
        let actual = CredentialStorage::from_legacy(encrypted, keyring);
        let expected = expected_storage(encrypted, keyring);
        prop_assert_eq!(actual, expected);
    }
}

// Property: setting then reading a storage choice via the helper API
// preserves the value for every backend.
proptest! {
    #[test]
    fn storage_round_trip(
        kdbx in 0u8..3,
        bw in 0u8..3,
        op in 0u8..3,
        pb in 0u8..3,
    ) {
        let to_storage = |n: u8| match n {
            1 => CredentialStorage::EncryptedFile,
            2 => CredentialStorage::SystemKeyring,
            _ => CredentialStorage::None,
        };
        let kdbx_choice = to_storage(kdbx);
        let bw_choice = to_storage(bw);
        let op_choice = to_storage(op);
        let pb_choice = to_storage(pb);

        let mut settings = SecretSettings::default();
        settings.set_kdbx_storage(kdbx_choice);
        settings.set_bitwarden_storage(bw_choice);
        settings.set_onepassword_storage(op_choice);
        settings.set_passbolt_storage(pb_choice);

        // Encrypted-file selections need a sentinel blob to make the read
        // back report `EncryptedFile`. The production GUI populates this
        // blob via `encrypt_password()` before save; tests populate it
        // directly to focus on the read/write API.
        if kdbx_choice == CredentialStorage::EncryptedFile {
            settings.kdbx_password_encrypted = Some("placeholder".to_string());
        }
        if bw_choice == CredentialStorage::EncryptedFile {
            settings.bitwarden_password_encrypted = Some("placeholder".to_string());
        }
        if op_choice == CredentialStorage::EncryptedFile {
            settings.onepassword_service_account_token_encrypted =
                Some("placeholder".to_string());
        }
        if pb_choice == CredentialStorage::EncryptedFile {
            settings.passbolt_passphrase_encrypted = Some("placeholder".to_string());
        }

        prop_assert_eq!(settings.kdbx_storage(), kdbx_choice);
        prop_assert_eq!(settings.bitwarden_storage(), bw_choice);
        prop_assert_eq!(settings.onepassword_storage(), op_choice);
        prop_assert_eq!(settings.passbolt_storage(), pb_choice);
    }
}

// Property: legacy configs that combine both `*_save_to_keyring = true`
// and an encrypted blob deterministically prefer the keyring choice. This
// guards the resolution of conflicting legacy data the user could have
// produced before the 3-state UI.
proptest! {
    #[test]
    fn legacy_conflict_prefers_keyring(
        encrypted in any::<bool>(),
    ) {
        let mut settings = SecretSettings::default();
        settings.kdbx_save_to_keyring = true;
        settings.bitwarden_save_to_keyring = true;
        settings.onepassword_save_to_keyring = true;
        settings.passbolt_save_to_keyring = true;
        if encrypted {
            settings.kdbx_password_encrypted = Some("legacy".to_string());
            settings.bitwarden_password_encrypted = Some("legacy".to_string());
            settings.onepassword_service_account_token_encrypted = Some("legacy".to_string());
            settings.passbolt_passphrase_encrypted = Some("legacy".to_string());
        }

        prop_assert_eq!(settings.kdbx_storage(), CredentialStorage::SystemKeyring);
        prop_assert_eq!(settings.bitwarden_storage(), CredentialStorage::SystemKeyring);
        prop_assert_eq!(
            settings.onepassword_storage(),
            CredentialStorage::SystemKeyring
        );
        prop_assert_eq!(settings.passbolt_storage(), CredentialStorage::SystemKeyring);
    }
}

// Property: switching to `None` clears both legacy fields, regardless of
// their previous state. This is the path that "Don't save" must take.
proptest! {
    #[test]
    fn switching_to_none_clears_legacy_fields(
        encrypted_was in any::<bool>(),
        keyring_was in any::<bool>(),
    ) {
        let mut settings = SecretSettings::default();
        settings.kdbx_password_encrypted = encrypted_was.then(|| "x".to_string());
        settings.kdbx_save_to_keyring = keyring_was;
        settings.bitwarden_password_encrypted = encrypted_was.then(|| "x".to_string());
        settings.bitwarden_save_to_keyring = keyring_was;
        settings.onepassword_service_account_token_encrypted =
            encrypted_was.then(|| "x".to_string());
        settings.onepassword_save_to_keyring = keyring_was;
        settings.passbolt_passphrase_encrypted = encrypted_was.then(|| "x".to_string());
        settings.passbolt_save_to_keyring = keyring_was;

        settings.set_kdbx_storage(CredentialStorage::None);
        settings.set_bitwarden_storage(CredentialStorage::None);
        settings.set_onepassword_storage(CredentialStorage::None);
        settings.set_passbolt_storage(CredentialStorage::None);

        prop_assert!(settings.kdbx_password_encrypted.is_none());
        prop_assert!(!settings.kdbx_save_to_keyring);
        prop_assert!(settings.bitwarden_password_encrypted.is_none());
        prop_assert!(!settings.bitwarden_save_to_keyring);
        prop_assert!(
            settings
                .onepassword_service_account_token_encrypted
                .is_none()
        );
        prop_assert!(!settings.onepassword_save_to_keyring);
        prop_assert!(settings.passbolt_passphrase_encrypted.is_none());
        prop_assert!(!settings.passbolt_save_to_keyring);
    }
}

// Property: switching to `SystemKeyring` clears the encrypted blob (so
// the next save doesn't persist a stale machine-encrypted credential)
// and sets the keyring flag.
proptest! {
    #[test]
    fn switching_to_keyring_clears_encrypted(
        encrypted_was in any::<bool>(),
    ) {
        let mut settings = SecretSettings::default();
        if encrypted_was {
            settings.kdbx_password_encrypted = Some("x".to_string());
            settings.bitwarden_password_encrypted = Some("x".to_string());
            settings.onepassword_service_account_token_encrypted = Some("x".to_string());
            settings.passbolt_passphrase_encrypted = Some("x".to_string());
        }

        settings.set_kdbx_storage(CredentialStorage::SystemKeyring);
        settings.set_bitwarden_storage(CredentialStorage::SystemKeyring);
        settings.set_onepassword_storage(CredentialStorage::SystemKeyring);
        settings.set_passbolt_storage(CredentialStorage::SystemKeyring);

        prop_assert!(settings.kdbx_password_encrypted.is_none());
        prop_assert!(settings.kdbx_save_to_keyring);
        prop_assert!(settings.bitwarden_password_encrypted.is_none());
        prop_assert!(settings.bitwarden_save_to_keyring);
        prop_assert!(
            settings
                .onepassword_service_account_token_encrypted
                .is_none()
        );
        prop_assert!(settings.onepassword_save_to_keyring);
        prop_assert!(settings.passbolt_passphrase_encrypted.is_none());
        prop_assert!(settings.passbolt_save_to_keyring);
    }
}
