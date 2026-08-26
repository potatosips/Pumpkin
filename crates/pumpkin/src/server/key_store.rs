use std::time::Instant;

use num_bigint::BigInt;
use pumpkin_protocol::java::client::login::CEncryptionRequest;
use rsa::pkcs8::EncodePublicKey;
use rsa::{Pkcs1v15Encrypt, RsaPrivateKey};
use sha1::Sha1;
use sha2::Digest;
use tracing::{debug, error};

use crate::net::EncryptionError;

pub struct KeyStore {
    pub private_key: RsaPrivateKey,
    pub public_key_der: Box<[u8]>,
}

impl KeyStore {
    #[must_use]
    pub fn new() -> Self {
        let instant = Instant::now();
        debug!("Creating encryption keys...");
        let private_key = Self::generate_private_key();

        let public_key = private_key.to_public_key();

        let public_key_der = public_key
            .to_public_key_der()
            .map(|der| der.into_vec().into_boxed_slice())
            .unwrap_or_default();

        debug!("Created RSA keys, took {}ms", instant.elapsed().as_millis());

        Self {
            private_key,
            public_key_der,
        }
    }

    fn generate_private_key() -> RsaPrivateKey {
        let mut rng = rand::rng();

        RsaPrivateKey::new(&mut rng, 1024).unwrap_or_else(|_| {
            let mut fallback_rng = rand::rng();
            RsaPrivateKey::new(&mut fallback_rng, 1024).unwrap_or_else(|_| {
                error!("Failed to generate RSA key");
                std::process::exit(1);
            })
        })
    }

    pub fn encryption_request<'a>(
        &'a self,
        server_id: &'a str,
        verification_token: &'a [u8; 4],
        should_authenticate: bool,
    ) -> CEncryptionRequest<'a> {
        CEncryptionRequest::new(
            server_id,
            &self.public_key_der,
            verification_token,
            should_authenticate,
        )
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, EncryptionError> {
        let decrypted = self
            .private_key
            .decrypt(Pkcs1v15Encrypt, data)
            .map_err(|_| EncryptionError::FailedDecrypt)?;
        Ok(decrypted)
    }

    pub fn get_digest(&self, secret: &[u8]) -> String {
        auth_digest(
            &Sha1::new()
                .chain_update(secret)
                .chain_update(&self.public_key_der)
                .finalize(),
        )
    }
}

pub fn auth_digest(bytes: &[u8]) -> String {
    BigInt::from_signed_bytes_be(bytes).to_str_radix(16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minecraft_auth_digest_hex_format_parity() {
        // Positive big-endian byte array
        let pos_bytes = [0x00, 0x12, 0x34];
        assert_eq!(auth_digest(&pos_bytes), "1234");

        // Negative big-endian byte array (signed two's complement representation)
        let neg_bytes = [0x80, 0x00];
        assert_eq!(auth_digest(&neg_bytes), "-8000");

        // Known Minecraft auth digest test vector: "Notch" test vector
        let notch_hash = sha1::Sha1::digest(b"Notch");
        let notch_digest = auth_digest(&notch_hash);
        assert_eq!(notch_digest, "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48");
    }
}
