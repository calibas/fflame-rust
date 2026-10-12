//! Encrypted credential storage for desktop auto-login.
//!
//! Uses AES-256-GCM with a hardcoded key and a random nonce per
//! encryption. Both email and password are encrypted and stored as
//! base64. This is obfuscation (key is in the binary), not true security.

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
    /// Random 12-byte nonce the email was encrypted under (base64).
    /// Files written before `password_nonce` existed used it for the
    /// password too.
    pub nonce: String,
    /// Random 12-byte nonce the password was encrypted under (base64).
    ///
    /// GCM must never encrypt two messages under one key and nonce: the
    /// keystream repeats, so the XOR of the two ciphertexts is the XOR
    /// of the two plaintexts, and the authentication key leaks. The
    /// email and password once shared `nonce`. Absent in files saved
    /// before this field, which then decrypt with `nonce` as they were
    /// written; the next save writes both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password_nonce: Option<String>,
}

/// Encrypt email and password, returning a SavedCredentials struct.
pub fn save_credentials(email: &str, password: &str) -> Result<SavedCredentials, String> {
    let key = Key::<Aes256Gcm>::from_slice(AES_KEY);
    let cipher = Aes256Gcm::new(key);
    // One nonce per message -- see `SavedCredentials::password_nonce`.
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let password_nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let encrypted_email = cipher
        .encrypt(&nonce, email.as_bytes())
        .map_err(|e| format!("Failed to encrypt email: {}", e))?;

    let encrypted_password = cipher
        .encrypt(&password_nonce, password.as_bytes())
        .map_err(|e| format!("Failed to encrypt password: {}", e))?;

    Ok(SavedCredentials {
        encrypted_email: BASE64.encode(&encrypted_email),
        encrypted_password: BASE64.encode(&encrypted_password),
        nonce: BASE64.encode(nonce.as_slice()),
        password_nonce: Some(BASE64.encode(password_nonce.as_slice())),
    })
}

/// Decrypt saved credentials, returning (email, password).
pub fn load_credentials(saved: &SavedCredentials) -> Result<(String, String), String> {
    let key = Key::<Aes256Gcm>::from_slice(AES_KEY);
    let cipher = Aes256Gcm::new(key);

    let decode_nonce = |b64: &str| -> Result<Vec<u8>, String> {
        let bytes = BASE64
            .decode(b64)
            .map_err(|e| format!("Invalid nonce base64: {}", e))?;
        // `Nonce::from_slice` panics on any other length.
        if bytes.len() != 12 {
            return Err(format!("Invalid nonce length: {}", bytes.len()));
        }
        Ok(bytes)
    };
    let nonce_bytes = decode_nonce(&saved.nonce)?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    // A legacy file has no password nonce: it shared the email's.
    let password_nonce_bytes = match &saved.password_nonce {
        Some(b64) => decode_nonce(b64)?,
        None => nonce_bytes.clone(),
    };
    let password_nonce = Nonce::from_slice(&password_nonce_bytes);

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
        .decrypt(password_nonce, password_bytes.as_ref())
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

    /// The email and password are encrypted under different nonces:
    /// AES-GCM under a repeated key and nonce leaks the XOR of the two
    /// plaintexts.
    #[test]
    fn email_and_password_use_distinct_nonces() {
        let saved = save_credentials("a@b.com", "pass1").unwrap();
        let password_nonce = saved.password_nonce.as_deref().expect("new saves carry a password nonce");
        assert_ne!(saved.nonce, password_nonce);
        assert_eq!(BASE64.decode(password_nonce).unwrap().len(), 12);
    }

    /// A file written before `password_nonce` existed -- both fields
    /// encrypted under the one `nonce`, no `password_nonce` key --
    /// still decrypts.
    #[test]
    fn a_legacy_shared_nonce_file_still_decrypts() {
        let key = Key::<Aes256Gcm>::from_slice(AES_KEY);
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let json = serde_json::json!({
            "encrypted_email": BASE64.encode(cipher.encrypt(&nonce, b"old@example.com".as_ref()).unwrap()),
            "encrypted_password": BASE64.encode(cipher.encrypt(&nonce, b"old-pass".as_ref()).unwrap()),
            "nonce": BASE64.encode(nonce.as_slice()),
        });
        let saved: SavedCredentials = serde_json::from_value(json).unwrap();
        assert!(saved.password_nonce.is_none());
        let (email, password) = load_credentials(&saved).unwrap();
        assert_eq!((email.as_str(), password.as_str()), ("old@example.com", "old-pass"));

        // And a re-save writes the new shape.
        let resaved = serde_json::to_value(save_credentials(&email, &password).unwrap()).unwrap();
        assert!(resaved.get("password_nonce").is_some());
    }

    /// A malformed nonce is an error, not a panic.
    #[test]
    fn a_bad_nonce_length_is_an_error() {
        let mut saved = save_credentials("a@b.com", "pass1").unwrap();
        saved.password_nonce = Some(BASE64.encode([0u8; 5]));
        assert!(load_credentials(&saved).is_err());
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
