use base64::{engine::general_purpose::STANDARD as B64, Engine};
use ed25519_dalek::Signer;
use ed25519_dalek::Verifier;
use ed25519_dalek::{ed25519::signature::rand_core::OsRng, Signature, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Serialize, Deserialize)]
pub struct NodeKeys {
    pub public_b64: String,
    pub private_b64: String,
}

impl NodeKeys {
    pub fn get_node_keys_path() -> std::path::PathBuf {
        let home_dir = Path::new(".hushnet");
        if !home_dir.exists() {
            std::fs::create_dir_all(home_dir).expect("Could not create home directory");
        }
        home_dir.join("node_keys")
    }

    pub fn generate_and_save() -> anyhow::Result<Self> {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key: VerifyingKey = signing_key.verifying_key();

        let private_b64 = B64.encode(signing_key.to_bytes());
        let public_b64 = B64.encode(verifying_key.to_bytes());

        let keys = NodeKeys {
            public_b64,
            private_b64,
        };

        let path = Self::get_node_keys_path();
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(&path, serde_json::to_string_pretty(&keys)?)?;

        Ok(keys)
    }

    pub fn load_or_generate() -> anyhow::Result<Self> {
        let path = Self::get_node_keys_path();
        if path.exists() {
            let data = fs::read_to_string(&path)?;
            let keys: NodeKeys = serde_json::from_str(&data)?;
            Ok(keys)
        } else {
            Self::generate_and_save()
        }
    }

    pub fn signing_key(&self) -> anyhow::Result<SigningKey> {
        let bytes = B64.decode(&self.private_b64)?;
        let key_bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid private key length (expected 32 bytes)"))?;

        Ok(SigningKey::from_bytes(&key_bytes))
    }

    #[allow(dead_code)]
    pub fn verifying_key(&self) -> anyhow::Result<VerifyingKey> {
        let bytes = B64.decode(&self.public_b64)?;
        let key_bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid public key length (expected 32 bytes)"))?;
        let vk = VerifyingKey::from_bytes(&key_bytes).map_err(|e| anyhow::anyhow!(e))?;
        Ok(vk)
    }

    #[allow(dead_code)]
    pub fn sign_message(&self, message: &[u8]) -> anyhow::Result<String> {
        let signing_key = self.signing_key()?;
        let signature: Signature = signing_key.sign(message);
        Ok(B64.encode(signature.to_bytes()))
    }

    #[allow(dead_code)]
    pub fn verify_message(&self, message: &[u8], signature_b64: &str) -> anyhow::Result<bool> {
        let vk = self.verifying_key()?;
        let sig_bytes = B64.decode(signature_b64)?;
        let key_bytes: [u8; 64] = sig_bytes
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid signature length (expected 64 bytes)"))?;
        let sig = Signature::from_bytes(&key_bytes);
        Ok(vk.verify(message, &sig).is_ok())
    }
}
