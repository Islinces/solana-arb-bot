use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use anyhow::anyhow;
use argon2::Argon2;
use rand::RngCore;
use rand_core::OsRng;
use solana_sdk::signature::Keypair;
use std::path::Path;
use std::fs;

pub struct KeypairVault {
    encrypted_data: Vec<u8>,
    salt: [u8; 16],
    nonce: [u8; 12],
}

impl KeypairVault {
    pub fn create(password: &str, keypair: &Keypair) -> anyhow::Result<Self> {
        // Generate random salt for Argon2
        let mut salt_bytes = [0u8; 16];
        OsRng.fill_bytes(&mut salt_bytes);

        // Configure Argon2 with strong parameters
        // Configure Argon2 with high security parameters
        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id, // Argon2id variant - best for password hashing
            argon2::Version::V0x13,      // Latest version
            argon2::Params::new(
                128 * 1024, // m_cost: 128 MiB (significantly increases memory hardness)
                3,          // t_cost: 3 iterations (increased time cost)
                4,          // p_cost: 4 parallel threads (better for modern CPUs)
                Some(32),   // 32 bytes output for AES-256
            )
            .unwrap(),
        );

        // Generate encryption key using Argon2
        let mut encryption_key = [0u8; 32];
        argon2
            .hash_password_into(password.as_bytes(), &salt_bytes, &mut encryption_key)
            .unwrap();

        // Generate random nonce for AES-GCM
        let mut nonce = [0u8; 12];
        OsRng.fill_bytes(&mut nonce);

        // Create AES-GCM cipher
        let cipher = Aes256Gcm::new_from_slice(&encryption_key)?;

        // Serialize the keypair to bytes
        let keypair_bytes = keypair.to_bytes();

        // Encrypt the keypair
        let encrypted_data = cipher
            .encrypt(Nonce::from_slice(&nonce), keypair_bytes.as_ref())
            .map_err(|e| anyhow!("Encryption failed: {}", e))?;

        Ok(Self {
            encrypted_data,
            salt: salt_bytes[0..16].try_into().unwrap(),
            nonce,
        })
    }

    pub fn save(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.salt);
        data.extend_from_slice(&self.nonce);
        data.extend_from_slice(&self.encrypted_data);
        fs::write(path, data)?;
        Ok(())
    }

    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let data = fs::read(path)?;

        if data.len() < 28 {
            // 16 (salt) + 12 (nonce)
            return Err(anyhow!("Invalid vault file format"));
        }

        let mut salt = [0u8; 16];
        let mut nonce = [0u8; 12];

        salt.copy_from_slice(&data[0..16]);
        nonce.copy_from_slice(&data[16..28]);

        let encrypted_data = data[28..].to_vec();

        Ok(Self {
            encrypted_data,
            salt,
            nonce,
        })
    }

    pub fn decrypt(&self, password: &str) -> anyhow::Result<Keypair> {
        // Configure Argon2
        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id, // Argon2id variant - best for password hashing
            argon2::Version::V0x13,      // Latest version
            argon2::Params::new(
                128 * 1024, // m_cost: 128 MiB (significantly increases memory hardness)
                3,          // t_cost: 3 iterations (increased time cost)
                4,          // p_cost: 4 parallel threads (better for modern CPUs)
                Some(32),   // 32 bytes output for AES-256
            )
            .unwrap(),
        );

        // Recreate encryption key
        let mut encryption_key = [0u8; 32];
        argon2
            .hash_password_into(password.as_bytes(), &self.salt, &mut encryption_key)
            .unwrap();

        // Create cipher
        let cipher = Aes256Gcm::new_from_slice(&encryption_key)?;

        // Decrypt the keypair
        let keypair_bytes = cipher
            .decrypt(Nonce::from_slice(&self.nonce), self.encrypted_data.as_ref())
            .map_err(|e| anyhow!("Decryption failed: {}", e))?;

        // Convert back to Keypair
        let keypair = Keypair::from_bytes(&keypair_bytes)
            .map_err(|e| anyhow!("Invalid keypair data: {}", e))?;
        Ok(keypair)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::str::FromStr;
    use tempfile::tempdir;

    #[test]
    fn test_keypair_vault() -> anyhow::Result<()> {
        let temp_dir = tempdir().unwrap();
        let vault_path = temp_dir.path().join("keypair_vault.bin");
        // // Create a test keypair
        // let original_keypair = Keypair::new();
        let password = "test_password123";
        // // Create and save vault
        // let vault = KeypairVault::create(password, &original_keypair)?;
        // vault.save(&vault_path)?;
        let vault_path=PathBuf::from_str("./src/keypair/keypair_vault.bin").unwrap();
        // Load and decrypt vault
        let loaded_vault = KeypairVault::load(&vault_path)?;
        let decrypted_keypair = loaded_vault.decrypt(password)?;

        // Verify keypairs match
        // assert_eq!(original_keypair.to_bytes(), decrypted_keypair.to_bytes());
        Ok(())
    }

    #[test]
    fn test_wrong_password() {
        let keypair = Keypair::new();
        let vault = KeypairVault::create("correct_password", &keypair).unwrap();
        assert!(vault.decrypt("wrong_password").is_err());
    }
}
