//! Encrypted credential storage for desktop auto-login.
//!
//! Uses AES-256-GCM with a hardcoded key and random nonce.
//! Both email and password are encrypted and stored as base64.
//! This is obfuscation (key is in the binary), not true security.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    AeadCore, Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};

/// AES-256 key (32 bytes). Hardcoded in binary — obfuscation, not real security.
const AES_KEY: &[u8; 32] = b"\x9a\x3f\x7c\x1d\xe5\x8b\x42\xf0\xd6\x73\xa1\x0e\xb9\x54\x28\x6f\xc3\x17\x90\x4d\xe8\x2a\x65\xfb\x0c\x81\xd7\x3e\xa4\x59\xb6\x12";

/// Encrypted credentials stored in system_settings.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedCredentials {
    /// AES-256-GCM encrypted email (base64)
    pub encrypted_email: String,
    /// AES-256-GCM encrypted password (base64)
    pub encrypted_password: String,
    /// Random 12-byte nonce used for encryption (base64)
    pub nonce: String,
}

/// Encrypt email and password, returning a SavedCredentials struct.
pub fn save_credentials(email: &str, password: &str) -> Result<SavedCredentials, String> {
    let key = Key::<Aes256Gcm>::from_slice(AES_KEY);
    let cipher = Aes256Gcm::new(key);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let encrypted_email = cipher
        .encrypt(&nonce, email.as_bytes())
        .map_err(|e| format!("Failed to encrypt email: {}", e))?;

    let encrypted_password = cipher
        .encrypt(&nonce, password.as_bytes())
        .map_err(|e| format!("Failed to encrypt password: {}", e))?;

    Ok(SavedCredentials {
        encrypted_email: BASE64.encode(&encrypted_email),
        encrypted_password: BASE64.encode(&encrypted_password),
        nonce: BASE64.encode(nonce.as_slice()),
    })
}

/// Decrypt saved credentials, returning (email, password).
pub fn load_credentials(saved: &SavedCredentials) -> Result<(String, String), String> {
    let key = Key::<Aes256Gcm>::from_slice(AES_KEY);
    let cipher = Aes256Gcm::new(key);

    let nonce_bytes = BASE64
        .decode(&saved.nonce)
        .map_err(|e| format!("Invalid nonce base64: {}", e))?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let email_bytes = BASE64
        .decode(&saved.encrypted_email)
        .map_err(|e| format!("Invalid email base64: {}", e))?;
    let password_bytes = BASE64
        .decode(&saved.encrypted_password)
        .map_err(|e| format!("Invalid password base64: {}", e))?;

    let email = cipher
        .decrypt(nonce, email_bytes.as_ref())
        .map_err(|e| format!("Failed to decrypt email: {}", e))?;
    let password = cipher
        .decrypt(nonce, password_bytes.as_ref())
        .map_err(|e| format!("Failed to decrypt password: {}", e))?;

    let email = String::from_utf8(email).map_err(|e| format!("Invalid email UTF-8: {}", e))?;
    let password =
        String::from_utf8(password).map_err(|e| format!("Invalid password UTF-8: {}", e))?;

    Ok((email, password))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_encrypt_decrypt() {
        let email = "user@example.com";
        let password = "s3cret!P@ss";

        let saved = save_credentials(email, password).expect("encrypt failed");
        let (dec_email, dec_password) = load_credentials(&saved).expect("decrypt failed");

        assert_eq!(dec_email, email);
        assert_eq!(dec_password, password);
    }

    #[test]
    fn different_nonce_each_save() {
        let saved1 = save_credentials("a@b.com", "pass1").unwrap();
        let saved2 = save_credentials("a@b.com", "pass1").unwrap();

        // Nonces should differ (random)
        assert_ne!(saved1.nonce, saved2.nonce);
        // Ciphertext should also differ due to different nonces
        assert_ne!(saved1.encrypted_email, saved2.encrypted_email);
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let saved = save_credentials("test@test.com", "password").unwrap();

        let mut tampered = saved.clone();
        // Flip a byte in the encrypted email
        let mut bytes = BASE64.decode(&tampered.encrypted_email).unwrap();
        bytes[0] ^= 0xFF;
        tampered.encrypted_email = BASE64.encode(&bytes);

        assert!(load_credentials(&tampered).is_err());
    }

    #[test]
    fn empty_strings() {
        let saved = save_credentials("", "").expect("encrypt empty failed");
        let (email, password) = load_credentials(&saved).expect("decrypt empty failed");
        assert_eq!(email, "");
        assert_eq!(password, "");
    }

    #[test]
    fn unicode_credentials() {
        let email = "user@例え.jp";
        let password = "パスワード🔑";

        let saved = save_credentials(email, password).unwrap();
        let (dec_email, dec_password) = load_credentials(&saved).unwrap();

        assert_eq!(dec_email, email);
        assert_eq!(dec_password, password);
    }
}
