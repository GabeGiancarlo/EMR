//! Authenticated media encryption. Database backups intentionally contain no key.
use crate::{ApiError, Result};
use actix_web::http::StatusCode;
use ring::{
    aead,
    rand::{SecureRandom, SystemRandom},
};
use uuid::Uuid;

const KEY_ID: &str = "local-v1";
pub struct Sealed {
    pub key_id: &'static str,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub sha256: String,
}
fn unavailable() -> ApiError {
    ApiError {status:StatusCode::SERVICE_UNAVAILABLE,code:"media_vault",message:"The local media encryption key is unavailable or invalid. Configure the media vault before recording.".into()}
}
fn key_bytes() -> Result<[u8; 32]> {
    let value = std::env::var("NEXUS_MEDIA_KEY_HEX").map_err(|_| unavailable())?;
    decode_key(&value).ok_or_else(unavailable)
}
fn decode_key(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|v| v.is_ascii_hexdigit()) {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (i, b) in bytes.iter_mut().enumerate() {
        *b = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(bytes)
}
pub fn configured() -> bool {
    key_bytes().is_ok()
}
fn aad(org: Uuid, recording: Uuid, track: Uuid, sequence: i32) -> Vec<u8> {
    format!("nexus-media-v1:{org}:{recording}:{track}:{sequence}").into_bytes()
}
pub fn seal(
    org: Uuid,
    recording: Uuid,
    track: Uuid,
    sequence: i32,
    plain: &[u8],
) -> Result<Sealed> {
    encrypt(&key_bytes()?, &aad(org, recording, track, sequence), plain)
}
fn encrypt(key: &[u8; 32], context: &[u8], plain: &[u8]) -> Result<Sealed> {
    let key = aead::LessSafeKey::new(
        aead::UnboundKey::new(&aead::AES_256_GCM, key).map_err(|_| unavailable())?,
    );
    let mut nonce = [0u8; 12];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| unavailable())?;
    let mut ciphertext = plain.to_vec();
    key.seal_in_place_append_tag(
        aead::Nonce::assume_unique_for_key(nonce),
        aead::Aad::from(context),
        &mut ciphertext,
    )
    .map_err(|_| unavailable())?;
    Ok(Sealed {
        key_id: KEY_ID,
        nonce: nonce.to_vec(),
        ciphertext,
        sha256: crate::sha256_hex(plain),
    })
}
pub fn open(
    org: Uuid,
    recording: Uuid,
    track: Uuid,
    sequence: i32,
    key_id: &str,
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    if key_id != KEY_ID {
        return Err(unavailable());
    }
    decrypt(
        &key_bytes()?,
        &aad(org, recording, track, sequence),
        nonce,
        ciphertext,
    )
}
fn decrypt(key: &[u8; 32], context: &[u8], nonce: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>> {
    let nonce: [u8; 12] = nonce.try_into().map_err(|_| unavailable())?;
    let key = aead::LessSafeKey::new(
        aead::UnboundKey::new(&aead::AES_256_GCM, key).map_err(|_| unavailable())?,
    );
    let mut bytes = ciphertext.to_vec();
    let plain = key
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(context),
            &mut bytes,
        )
        .map_err(|_| ApiError {
            status: StatusCode::CONFLICT,
            code: "media_integrity",
            message: "Stored media failed its integrity check. Playback was stopped.".into(),
        })?;
    Ok(plain.to_vec())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ciphertext_binds_context_and_rejects_tampering() {
        let key = [7u8; 32];
        let sealed = encrypt(&key, b"org:record:track:0", b"synthetic media").unwrap();
        assert_ne!(sealed.ciphertext, b"synthetic media");
        assert_eq!(
            decrypt(
                &key,
                b"org:record:track:0",
                &sealed.nonce,
                &sealed.ciphertext
            )
            .unwrap(),
            b"synthetic media"
        );
        assert!(decrypt(
            &key,
            b"other:record:track:0",
            &sealed.nonce,
            &sealed.ciphertext
        )
        .is_err());
        assert!(decrypt(
            &[8u8; 32],
            b"org:record:track:0",
            &sealed.nonce,
            &sealed.ciphertext
        )
        .is_err());
        let mut corrupt = sealed.ciphertext.clone();
        corrupt[0] ^= 1;
        assert!(decrypt(&key, b"org:record:track:0", &sealed.nonce, &corrupt).is_err());
        assert_ne!(
            sealed.nonce,
            encrypt(&key, b"org:record:track:0", b"synthetic media")
                .unwrap()
                .nonce
        );
    }
    #[test]
    fn keys_require_exact_hex() {
        assert!(decode_key(&"ab".repeat(32)).is_some());
        for s in ["x".repeat(64), "ab".repeat(31), "ab".repeat(33)] {
            assert!(decode_key(&s).is_none());
        }
    }
}
