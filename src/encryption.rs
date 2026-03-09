use aes::Aes256;
use cbc::cipher::{block_padding::Pkcs7, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use cbc::{Decryptor, Encryptor};
use rand::{rng, Rng};
use scrypt::{scrypt, Params};
use thiserror::Error;

type Aes256CbcEnc = Encryptor<Aes256>;
type Aes256CbcDec = Decryptor<Aes256>;

#[derive(Error, Debug)]
pub enum EncryptionError {
    #[error("Server error")]
    ServerError,
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Invalid data format")]
    InvalidData,
    #[error("Environment variable error: {0}")]
    EnvError(String),
}

pub struct Encryption {
    key: [u8; 32], // Pre-derived key
}

impl Encryption {
    /// Creates a new EncryptionService instance
    ///
    /// # Errors
    ///
    /// Returns an error if the secret is less than 20 characters long
    pub fn new(secret: String) -> Result<Self, EncryptionError> {
        if secret.len() < 20 {
            return Err(EncryptionError::EnvError(
                "the secret cannot be null and should have at least 20 characters to be considered secure".to_string()
            ));
        }

        // Derive key once during initialization with much faster parameters
        let key = Self::derive_key_from_secret(&secret)?;

        Ok(Self { key })
    }

    /// Derives a key from the secret using faster scrypt parameters
    fn derive_key_from_secret(secret: &str) -> Result<[u8; 32], EncryptionError> {
        let mut key = [0u8; 32];
        // Use much faster parameters: N=2^12 instead of 2^14 (4x faster)
        let params = Params::new(12, 8, 1, 32).map_err(|_| EncryptionError::ServerError)?;

        scrypt(secret.as_bytes(), b"token", &params, &mut key)
            .map_err(|_| EncryptionError::ServerError)?;

        Ok(key)
    }

    /// Turns cleartext into ciphertext
    ///
    /// # Arguments
    ///
    /// * `data` - The string data to encrypt
    ///
    /// # Returns
    ///
    /// Returns the encrypted data as a hex string with the IV prepended
    ///
    /// # Errors
    ///
    /// Returns `EncryptionError::ServerError` if encryption fails
    pub fn encrypt(&self, data: &str) -> Result<String, EncryptionError> {
        if data.is_empty() {
            return Err(EncryptionError::InvalidData);
        }

        // Use pre-derived key
        let key = &self.key;

        // Generate random IV (16 bytes for AES-256-CBC)
        let mut iv = [0u8; 16];
        rng().fill(&mut iv);

        // Create cipher
        let cipher = Aes256CbcEnc::new(key.into(), &iv.into());

        // Encrypt the data
        let mut buffer = data.as_bytes().to_vec();
        // Add padding space - AES block size is 16 bytes
        buffer.resize(buffer.len() + 16, 0);
        let encrypted_len = cipher
            .encrypt_padded_mut::<Pkcs7>(&mut buffer, data.len())
            .map_err(|_| EncryptionError::ServerError)?
            .len();
        buffer.truncate(encrypted_len);

        // Combine IV and encrypted data as hex string
        let iv_hex = hex::encode(iv);
        let encrypted_hex = hex::encode(&buffer);

        Ok(format!("{}{}", iv_hex, encrypted_hex))
    }

    /// Turns ciphertext into cleartext
    ///
    /// # Arguments
    ///
    /// * `data` - The hex-encoded encrypted data with IV prepended
    ///
    /// # Returns
    ///
    /// Returns the decrypted string
    ///
    /// # Errors
    ///
    /// Returns `EncryptionError::Unauthorized` if decryption fails or data format is invalid
    pub fn decrypt(&self, data: &str) -> Result<String, EncryptionError> {
        if data.is_empty() {
            return Err(EncryptionError::InvalidData);
        }

        // IV in hex format will always have 32 characters
        if data.len() < 32 || data.len() % 2 != 0 {
            return Err(EncryptionError::Unauthorized);
        }

        // Use pre-derived key
        let key = &self.key;

        // Extract IV (first 32 hex characters = 16 bytes)
        let iv_hex = &data[..32];
        let encrypted_hex = &data[32..];

        // Decode hex strings
        let iv = hex::decode(iv_hex).map_err(|_| EncryptionError::Unauthorized)?;
        let encrypted = hex::decode(encrypted_hex).map_err(|_| EncryptionError::Unauthorized)?;

        if iv.len() != 16 {
            return Err(EncryptionError::Unauthorized);
        }

        // Convert IV to fixed-size array
        let iv_array: [u8; 16] = iv.try_into().map_err(|_| EncryptionError::Unauthorized)?;

        // Create cipher
        let cipher = Aes256CbcDec::new(key.into(), &iv_array.into());

        // Decrypt the data
        let mut encrypted_copy = encrypted.clone();
        let decrypted = cipher
            .decrypt_padded_mut::<Pkcs7>(&mut encrypted_copy)
            .map_err(|_| EncryptionError::Unauthorized)?;

        // Convert to string
        String::from_utf8(decrypted.to_vec()).map_err(|_| EncryptionError::Unauthorized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        // Set up test environment
        let secret = "this_is_a_very_long_secret_key_for_testing_purposes".to_string();

        let service = Encryption::new(secret).expect("Failed to create encryption service");
        let original_data = "Hello, World!";

        // Test encryption
        let encrypted = service.encrypt(original_data).expect("Failed to encrypt");

        assert!(!encrypted.is_empty());
        assert!(encrypted.len() > 32); // Should have IV + encrypted data

        // Test decryption
        let decrypted = service.decrypt(&encrypted).expect("Failed to decrypt");
        assert_eq!(decrypted, original_data);
    }

    #[test]
    fn test_invalid_secret() {
        let secret = "short".to_string();
        let result = Encryption::new(secret);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_invalid_data() {
        let secret = "this_is_a_very_long_secret_key_for_testing_purposes".to_string();
        let service = Encryption::new(secret).expect("Failed to create encryption service");

        // Test with invalid hex data
        let result = service.decrypt("invalid_hex_data");
        assert!(matches!(result, Err(EncryptionError::Unauthorized)));

        // Test with too short data
        let result = service.decrypt("short");
        assert!(matches!(result, Err(EncryptionError::Unauthorized)));
    }
}
