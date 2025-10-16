use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::{Verifier, VerifyingKey, Signature};

pub fn verify_signed_prekey_signature(
    identity_pubkey_b64: &str,
    spk_pub_b64: &str,
    spk_sig_b64: &str,
) -> Result<(), String> {
    let identity_bytes = general_purpose::STANDARD
        .decode(identity_pubkey_b64)
        .map_err(|_| "Invalid Base64 in identity_pubkey")?;

    let spk_pubkey = general_purpose::STANDARD
        .decode(spk_pub_b64)
        .map_err(|_| "Invalid Base64 in signed_prekey.key")?;
    let spk_sig = general_purpose::STANDARD
        .decode(spk_sig_b64)
        .map_err(|_| "Invalid Base64 in signed_prekey.signature")?;

let ik_pub =
        VerifyingKey::try_from(&identity_bytes[..]).map_err(|_| "Invalid identity_pubkey bytes")?;

    // Signature Ed25519
    let signature =
        Signature::try_from(&spk_sig[..]).map_err(|_| "Invalid signed_prekey.signature bytes")?;

    // Vérification Ed25519
    ik_pub
        .verify(&spk_pubkey, &signature)
        .map_err(|_| "Invalid signed_prekey signature")?;

    Ok(())
}
