//! Bedrock encryption primitives: ECDH (P-384), ES384 handshake JWT and
//! AES-256-CFB8 with PocketMine's checksum scheme.
//!
//! Mirrors `pocketmine\network\mcpe\encryption\EncryptionContext`,
//! `EncryptionUtils` and `JwtUtils` from PocketMine-MP.

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use openssl::ec::{EcGroup, EcKey};
use openssl::hash::{Hasher, MessageDigest};
use openssl::memcmp;
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private, Public};
use openssl::rand::rand_bytes;
use openssl::sign::{Signer, Verifier};
use openssl::symm::{Cipher, Crypter, Mode};
use serde_json::json;

/// Generates the P-384 key used for the handshake and server JWT signing.
pub fn generate_server_key() -> Result<PKey<Private>, String> {
    let group = EcGroup::from_curve_name(Nid::SECP384R1).map_err(|e| e.to_string())?;
    let key = EcKey::generate(&group).map_err(|e| e.to_string())?;
    PKey::from_ec_key(key).map_err(|e| e.to_string())
}

/// Derives the 48-byte ECDH shared secret with the client public key (DER/SPKI).
pub fn derive_shared_secret(
    server_key: &PKey<Private>,
    client_public_der: &[u8],
) -> Result<Vec<u8>, String> {
    let peer = PKey::public_key_from_der(client_public_der)
        .map_err(|e| format!("invalid client public key: {e}"))?;
    let mut deriver = openssl::derive::Deriver::new(server_key).map_err(|e| e.to_string())?;
    deriver.set_peer(&peer).map_err(|e| e.to_string())?;
    deriver.derive_to_vec().map_err(|e| e.to_string())
}

/// `SHA256(salt || sharedSecret)`, the AES key used by Bedrock.
pub fn derive_aes_key(shared_secret: &[u8], salt: &[u8]) -> [u8; 32] {
    let mut input = Vec::with_capacity(salt.len() + shared_secret.len());
    input.extend_from_slice(salt);
    input.extend_from_slice(shared_secret);
    let digest = openssl::hash::hash(MessageDigest::sha256(), &input).expect("sha256");
    let mut key = [0u8; 32];
    key.copy_from_slice(&digest);
    key
}

pub fn random_salt() -> [u8; 16] {
    let mut salt = [0u8; 16];
    rand_bytes(&mut salt).expect("openssl random");
    salt
}

/// Creates the `ServerToClientHandshake` JWT. Header contains the server public
/// key DER in `x5u`, payload contains the base64 salt; signature is ES384
/// encoded as raw `R || S` (96 bytes), as Bedrock expects.
pub fn create_handshake_jwt(server_key: &PKey<Private>, salt: &[u8]) -> Result<String, String> {
    let public_der = server_key.public_key_to_der().map_err(|e| e.to_string())?;
    let header = json!({
        "alg": "ES384",
        "x5u": STANDARD.encode(public_der),
    });
    let payload = json!({
        "salt": STANDARD.encode(salt),
    });
    let body = format!(
        "{}.{}",
        URL_SAFE_NO_PAD.encode(header.to_string()),
        URL_SAFE_NO_PAD.encode(payload.to_string())
    );

    let mut signer = Signer::new(MessageDigest::sha384(), server_key).map_err(|e| e.to_string())?;
    signer.update(body.as_bytes()).map_err(|e| e.to_string())?;
    let der = signer.sign_to_vec().map_err(|e| e.to_string())?;
    let raw =
        der_signature_to_raw(&der).ok_or_else(|| "could not convert DER signature".to_string())?;
    Ok(format!("{}.{}", body, URL_SAFE_NO_PAD.encode(raw)))
}

/// Parses the handshake JWT header/payload (used by clients and tests).
pub fn parse_handshake_jwt(token: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let mut parts = token.split('.');
    let header = URL_SAFE_NO_PAD.decode(parts.next()?).ok()?;
    let payload = URL_SAFE_NO_PAD.decode(parts.next()?).ok()?;
    let header: serde_json::Value = serde_json::from_slice(&header).ok()?;
    let payload: serde_json::Value = serde_json::from_slice(&payload).ok()?;
    let public_der = STANDARD.decode(header.get("x5u")?.as_str()?).ok()?;
    let salt = STANDARD.decode(payload.get("salt")?.as_str()?).ok()?;
    Some((public_der, salt))
}

/// Verifies an ES384 JWT signature given the public key DER.
pub fn verify_handshake_jwt(token: &str, public_der: &[u8]) -> bool {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    let body = format!("{}.{}", parts[0], parts[1]);
    let Ok(raw) = URL_SAFE_NO_PAD.decode(parts[2]) else {
        return false;
    };
    let Some(der) = raw_signature_to_der(&raw) else {
        return false;
    };
    let Ok(key) = PKey::public_key_from_der(public_der) else {
        return false;
    };
    let Ok(mut verifier) = Verifier::new(MessageDigest::sha384(), &key) else {
        return false;
    };
    verifier.update(body.as_bytes()).is_ok() && verifier.verify(&der).unwrap_or(false)
}

/// AES-256-CFB8 stream context with PMMP's per-direction checksum counter.
pub struct EncryptionContext {
    key: [u8; 32],
    encrypt_crypter: Crypter,
    decrypt_crypter: Crypter,
    encrypt_counter: u64,
    decrypt_counter: u64,
}

impl EncryptionContext {
    pub fn new(key: [u8; 32]) -> Result<Self, String> {
        let iv = &key[0..16];
        let mut encrypt_crypter =
            Crypter::new(Cipher::aes_256_cfb8(), Mode::Encrypt, &key, Some(iv))
                .map_err(|e| e.to_string())?;
        let mut decrypt_crypter =
            Crypter::new(Cipher::aes_256_cfb8(), Mode::Decrypt, &key, Some(iv))
                .map_err(|e| e.to_string())?;
        encrypt_crypter.pad(false);
        decrypt_crypter.pad(false);
        Ok(Self {
            key,
            encrypt_crypter,
            decrypt_crypter,
            encrypt_counter: 0,
            decrypt_counter: 0,
        })
    }

    pub fn encrypt(&mut self, payload: &[u8]) -> Vec<u8> {
        let checksum = self.checksum(self.encrypt_counter, payload);
        self.encrypt_counter = self.encrypt_counter.wrapping_add(1);
        let mut data = Vec::with_capacity(payload.len() + 8);
        data.extend_from_slice(payload);
        data.extend_from_slice(&checksum);

        let mut out = vec![0u8; data.len() + 32];
        let written = self
            .encrypt_crypter
            .update(&data, &mut out)
            .expect("cfb8 encrypt");
        out.truncate(written);
        out
    }

    pub fn decrypt(&mut self, encrypted: &[u8]) -> Result<Vec<u8>, String> {
        if encrypted.len() < 9 {
            return Err("encrypted payload too short".to_string());
        }
        let mut out = vec![0u8; encrypted.len() + 32];
        let written = self
            .decrypt_crypter
            .update(encrypted, &mut out)
            .map_err(|e| e.to_string())?;
        out.truncate(written);
        if out.len() < 8 {
            return Err("encrypted payload too short".to_string());
        }
        let split = out.len() - 8;
        let (payload, received) = out.split_at(split);
        let expected = self.checksum(self.decrypt_counter, payload);
        self.decrypt_counter = self.decrypt_counter.wrapping_add(1);
        if !memcmp::eq(received, &expected) {
            return Err("encrypted packet has an invalid checksum".to_string());
        }
        Ok(payload.to_vec())
    }

    fn checksum(&self, counter: u64, payload: &[u8]) -> [u8; 8] {
        let mut hasher = Hasher::new(MessageDigest::sha256()).expect("sha256");
        hasher.update(&counter.to_le_bytes()).expect("hash update");
        hasher.update(payload).expect("hash update");
        hasher.update(&self.key).expect("hash update");
        let digest = hasher.finish().expect("hash finish");
        let mut out = [0u8; 8];
        out.copy_from_slice(&digest[..8]);
        out
    }
}

// ---------------------------------------------------------------------------
// DER <-> raw ECDSA signature helpers
// ---------------------------------------------------------------------------

fn read_der_length(input: &[u8], index: &mut usize) -> Option<usize> {
    let first = *input.get(*index)?;
    *index += 1;
    if first & 0x80 == 0 {
        return Some(first as usize);
    }
    let count = (first & 0x7f) as usize;
    if count == 0 || count > 4 {
        return None;
    }
    let mut length = 0usize;
    for _ in 0..count {
        length = (length << 8) | *input.get(*index)? as usize;
        *index += 1;
    }
    Some(length)
}

pub fn der_signature_to_raw(der: &[u8]) -> Option<Vec<u8>> {
    if der.first()? != &0x30 {
        return None;
    }
    let mut index = 1usize;
    let sequence_len = read_der_length(der, &mut index)?;
    let sequence_end = index.checked_add(sequence_len)?;
    if sequence_end > der.len() {
        return None;
    }
    let mut parts = Vec::with_capacity(96);
    for _ in 0..2 {
        if *der.get(index)? != 0x02 {
            return None;
        }
        index += 1;
        let length = read_der_length(der, &mut index)?;
        let end = index.checked_add(length)?;
        let mut integer = der.get(index..end)?;
        index = end;
        while integer.len() > 1 && integer[0] == 0 {
            integer = &integer[1..];
        }
        if integer.len() > 48 {
            return None;
        }
        parts.extend(std::iter::repeat_n(0u8, 48 - integer.len()));
        parts.extend_from_slice(integer);
    }
    if index != sequence_end {
        return None;
    }
    Some(parts)
}

pub fn raw_signature_to_der(raw: &[u8]) -> Option<Vec<u8>> {
    if raw.len() != 96 {
        return None;
    }
    let mut sequence = Vec::with_capacity(104);
    for part in [&raw[..48], &raw[48..]] {
        let trimmed: &[u8] = {
            let mut slice = part;
            while slice.len() > 1 && slice[0] == 0 {
                slice = &slice[1..];
            }
            slice
        };
        let needs_zero = trimmed[0] & 0x80 != 0;
        sequence.push(0x02);
        sequence.push((trimmed.len() + needs_zero as usize) as u8);
        if needs_zero {
            sequence.push(0);
        }
        sequence.extend_from_slice(trimmed);
    }
    let mut der = Vec::with_capacity(sequence.len() + 2);
    der.push(0x30);
    der.push(sequence.len() as u8);
    der.extend_from_slice(&sequence);
    Some(der)
}

/// Public key DER for a private key (for diagnostics/tests).
pub fn public_key_der(key: &PKey<Private>) -> Result<Vec<u8>, String> {
    key.public_key_to_der().map_err(|e| e.to_string())
}

pub fn public_key_from_der(der: &[u8]) -> Result<PKey<Public>, String> {
    PKey::public_key_from_der(der).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openssl::ec::{EcGroup, EcKey};

    fn client_key() -> PKey<Private> {
        let group = EcGroup::from_curve_name(Nid::SECP384R1).unwrap();
        PKey::from_ec_key(EcKey::generate(&group).unwrap()).unwrap()
    }

    #[test]
    fn handshake_derives_matching_keys_and_verifies() {
        let server = generate_server_key().unwrap();
        let client = client_key();
        let client_der = public_key_der(&client).unwrap();
        let salt = [7u8; 16];

        let secret_server = derive_shared_secret(&server, &client_der).unwrap();
        let server_pub = public_key_from_der(&public_key_der(&server).unwrap()).unwrap();
        let secret_client = {
            let mut deriver = openssl::derive::Deriver::new(&client).unwrap();
            deriver.set_peer(&server_pub).unwrap();
            deriver.derive_to_vec().unwrap()
        };
        assert_eq!(secret_server, secret_client);
        assert_eq!(secret_server.len(), 48);

        let server_key = derive_aes_key(&secret_server, &salt);
        let client_key = derive_aes_key(&secret_client, &salt);
        assert_eq!(server_key, client_key);

        let jwt = create_handshake_jwt(&server, &salt).unwrap();
        let (jwt_pub, jwt_salt) = parse_handshake_jwt(&jwt).unwrap();
        assert_eq!(jwt_salt, salt);
        assert!(verify_handshake_jwt(&jwt, &jwt_pub));
    }

    #[test]
    fn encryption_context_roundtrip_is_stateful() {
        let key = [3u8; 32];
        let mut server = EncryptionContext::new(key).unwrap();
        let mut client = EncryptionContext::new(key).unwrap();

        for i in 0..5u8 {
            let payload = vec![i; 100 + i as usize];
            let encrypted = server.encrypt(&payload);
            assert_eq!(client.decrypt(&encrypted).unwrap(), payload);

            let reply = vec![i ^ 0xff; 40];
            let encrypted = client.encrypt(&reply);
            assert_eq!(server.decrypt(&encrypted).unwrap(), reply);
        }
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let key = [9u8; 32];
        let mut sender = EncryptionContext::new(key).unwrap();
        let mut receiver = EncryptionContext::new(key).unwrap();
        let mut encrypted = sender.encrypt(b"hello world");
        let last = encrypted.len() - 1;
        encrypted[last] ^= 0x01;
        assert!(receiver.decrypt(&encrypted).is_err());
    }

    #[test]
    fn der_and_raw_signature_roundtrip() {
        let server = generate_server_key().unwrap();
        let jwt = create_handshake_jwt(&server, &[1u8; 16]).unwrap();
        let signature = URL_SAFE_NO_PAD
            .decode(jwt.split('.').nth(2).unwrap())
            .unwrap();
        assert_eq!(signature.len(), 96);
        let der = raw_signature_to_der(&signature).unwrap();
        assert_eq!(der_signature_to_raw(&der).unwrap(), signature);
    }
}
