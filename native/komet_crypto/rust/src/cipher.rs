use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng, Payload};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use rand_core::RngCore;
use sha2::{Digest, Sha256};

use crate::alphabet;
use crate::error::CryptoError;

pub const KEY_LEN: usize = 32;

pub(crate) const MAGIC: u8 = 0x4B;
const VERSION_V1: u8 = 0x01;
const VERSION_V2: u8 = 0x02;
const HEADER_LEN: usize = 2;
pub(crate) const NONCE_LEN: usize = 12;
pub(crate) const TAG_LEN: usize = 16;
const MIN_BLOB_LEN_V1: usize = HEADER_LEN + NONCE_LEN + TAG_LEN;
const SALT_CONTEXT: &[u8] = b"komet-enc-v1";
const SALT_LEN: usize = 16;
const V2_HEADER_LEN: usize = HEADER_LEN + SALT_LEN;
const MIN_BLOB_LEN_V2: usize = V2_HEADER_LEN + NONCE_LEN + TAG_LEN;

const ARGON_MEMORY_KIB: u32 = 65536;
const ARGON_ITERATIONS: u32 = 3;
const ARGON_PARALLELISM: u32 = 1;

fn argon_key(input: &[u8], salt: &[u8]) -> Result<Vec<u8>, CryptoError> {
    let params = Params::new(
        ARGON_MEMORY_KIB,
        ARGON_ITERATIONS,
        ARGON_PARALLELISM,
        Some(KEY_LEN),
    )
    .map_err(|_| CryptoError::Internal)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    let mut key = vec![0u8; KEY_LEN];
    argon
        .hash_password_into(input, salt, &mut key)
        .map_err(|_| CryptoError::Internal)?;
    Ok(key)
}

pub fn derive_key(password: &str) -> Result<Vec<u8>, CryptoError> {
    if password.is_empty() {
        return Err(CryptoError::EmptyPassword);
    }
    let mut hasher = Sha256::new();
    hasher.update(SALT_CONTEXT);
    hasher.update(password.as_bytes());
    let digest = hasher.finalize();
    argon_key(password.as_bytes(), &digest[..SALT_LEN])
}

pub fn encrypt(plaintext: &str, key: &[u8]) -> Result<String, CryptoError> {
    let cipher = cipher_from(key)?;
    let header = [MAGIC, VERSION_V1];
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext.as_bytes(),
                aad: &header,
            },
        )
        .map_err(|_| CryptoError::Internal)?;

    let mut blob = Vec::with_capacity(HEADER_LEN + NONCE_LEN + sealed.len());
    blob.extend_from_slice(&header);
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&sealed);
    Ok(alphabet::encode(&blob))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn encrypt_v2(plaintext: &str, key: &[u8]) -> Result<String, CryptoError> {
    cipher_from(key)?;
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let subkey = argon_key(key, &salt)?;
    let sub = ChaCha20Poly1305::new(Key::from_slice(&subkey));

    let mut header = Vec::with_capacity(V2_HEADER_LEN);
    header.push(MAGIC);
    header.push(VERSION_V2);
    header.extend_from_slice(&salt);

    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let sealed = sub
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext.as_bytes(),
                aad: &header,
            },
        )
        .map_err(|_| CryptoError::Internal)?;

    let mut blob = Vec::with_capacity(V2_HEADER_LEN + NONCE_LEN + sealed.len());
    blob.extend_from_slice(&header);
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&sealed);
    Ok(alphabet::encode(&blob))
}

pub fn decrypt(text: &str, key: &[u8]) -> Result<String, CryptoError> {
    let blob = alphabet::decode(text)?;
    let cipher = cipher_from(key)?;
    match envelope_version(&blob) {
        Some(VERSION_V1) => {
            let nonce = Nonce::from_slice(&blob[HEADER_LEN..HEADER_LEN + NONCE_LEN]);
            let plain = cipher
                .decrypt(
                    nonce,
                    Payload {
                        msg: &blob[HEADER_LEN + NONCE_LEN..],
                        aad: &blob[..HEADER_LEN],
                    },
                )
                .map_err(|_| CryptoError::WrongKey)?;
            String::from_utf8(plain).map_err(|_| CryptoError::Malformed)
        }
        Some(VERSION_V2) => {
            let salt_start = HEADER_LEN;
            let salt_end = V2_HEADER_LEN;
            let subkey = argon_key(key, &blob[salt_start..salt_end])?;
            let sub = ChaCha20Poly1305::new(Key::from_slice(&subkey));
            let nonce = Nonce::from_slice(&blob[salt_end..salt_end + NONCE_LEN]);
            let plain = sub
                .decrypt(
                    nonce,
                    Payload {
                        msg: &blob[salt_end + NONCE_LEN..],
                        aad: &blob[..salt_end],
                    },
                )
                .map_err(|_| CryptoError::WrongKey)?;
            String::from_utf8(plain).map_err(|_| CryptoError::Malformed)
        }
        _ => Err(CryptoError::NotEncrypted),
    }
}

pub fn looks_encrypted(text: &str) -> bool {
    alphabet::decode(text)
        .map(|b| envelope_version(&b).is_some())
        .unwrap_or(false)
}

fn envelope_version(blob: &[u8]) -> Option<u8> {
    if blob.len() < HEADER_LEN || blob[0] != MAGIC {
        return None;
    }
    match blob[1] {
        VERSION_V1 if blob.len() >= MIN_BLOB_LEN_V1 => Some(VERSION_V1),
        VERSION_V2 if blob.len() >= MIN_BLOB_LEN_V2 => Some(VERSION_V2),
        _ => None,
    }
}

pub(crate) fn cipher_from(key: &[u8]) -> Result<ChaCha20Poly1305, CryptoError> {
    if key.len() != KEY_LEN {
        return Err(CryptoError::BadKeyLength);
    }
    Ok(ChaCha20Poly1305::new(Key::from_slice(key)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: [u8; KEY_LEN] = [7u8; KEY_LEN];
    const OTHER_KEY: [u8; KEY_LEN] = [8u8; KEY_LEN];

    #[test]
    fn roundtrip_returns_original_text() {
        for text in [
            "привет",
            "Hello, World!",
            "эмодзи 🔐 и переносы\nстрок",
            "a",
            "\u{0}\u{1}",
        ] {
            let encrypted = encrypt(text, &KEY).unwrap();
            assert_eq!(decrypt(&encrypted, &KEY).unwrap(), text);
        }
    }

    #[test]
    fn empty_plaintext_roundtrips() {
        let encrypted = encrypt("", &KEY).unwrap();
        assert_eq!(decrypt(&encrypted, &KEY).unwrap(), "");
    }

    #[test]
    fn ciphertext_looks_like_russian_words() {
        let encrypted = encrypt("привет", &KEY).unwrap();
        for ch in encrypted.chars() {
            assert!(
                ch == ' ' || alphabet::RU_LOWERCASE_WITHOUT_YO.contains(&ch),
                "unexpected char {ch}"
            );
        }
        assert!(encrypted.split(' ').count() > 1);
    }

    #[test]
    fn same_plaintext_produces_different_ciphertext() {
        let a = encrypt("одно и то же", &KEY).unwrap();
        let b = encrypt("одно и то же", &KEY).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn wrong_key_is_rejected() {
        let encrypted = encrypt("секрет", &KEY).unwrap();
        assert_eq!(decrypt(&encrypted, &OTHER_KEY), Err(CryptoError::WrongKey));
    }

    fn swap_letter_at(text: &str, position: usize) -> String {
        text.chars()
            .enumerate()
            .map(|(i, c)| {
                if i != position {
                    c
                } else if c == 'а' {
                    'б'
                } else {
                    'а'
                }
            })
            .collect()
    }

    #[test]
    fn tampered_ciphertext_is_rejected() {
        let encrypted = encrypt("секрет", &KEY).unwrap();
        let letters: Vec<usize> = encrypted
            .char_indices()
            .enumerate()
            .filter(|(_, (_, c))| *c != ' ')
            .map(|(i, _)| i)
            .collect();
        for position in &letters {
            assert!(
                decrypt(&swap_letter_at(&encrypted, *position), &KEY).is_err(),
                "tamper at {position} slipped through"
            );
        }
        let middle = letters[letters.len() / 2];
        assert_eq!(
            decrypt(&swap_letter_at(&encrypted, middle), &KEY),
            Err(CryptoError::WrongKey)
        );
    }

    #[test]
    fn whitespace_mangling_survives() {
        let encrypted = encrypt("пробелы декоративные", &KEY).unwrap();
        let no_spaces: String = encrypted.chars().filter(|c| *c != ' ').collect();
        let doubled = encrypted.replace(' ', "  ");
        let trimmed = format!("  {encrypted}\n");
        for variant in [no_spaces, doubled, trimmed] {
            assert_eq!(decrypt(&variant, &KEY).unwrap(), "пробелы декоративные");
        }
    }

    #[test]
    fn plain_text_is_not_mistaken_for_ciphertext() {
        for text in ["привет как дела", "ёлка", "hello", "", "12345"] {
            assert!(!looks_encrypted(text), "{text}");
        }
        let encrypted = encrypt("настоящее", &KEY).unwrap();
        assert!(looks_encrypted(&encrypted));
    }

    #[test]
    fn plain_text_decrypt_reports_not_encrypted() {
        assert_eq!(
            decrypt("привет как дела", &KEY),
            Err(CryptoError::NotEncrypted)
        );
    }

    #[test]
    fn bad_key_length_is_reported() {
        assert_eq!(encrypt("x", &[0u8; 8]), Err(CryptoError::BadKeyLength));
    }

    #[test]
    fn overhead_is_48_letters() {
        let encrypted = encrypt("", &KEY).unwrap();
        let letters = encrypted.chars().filter(|c| *c != ' ').count();
        assert_eq!(letters, 48);
    }

    #[test]
    fn key_derivation_is_deterministic_and_password_bound() {
        let a = derive_key("общий ключ").unwrap();
        let b = derive_key("общий ключ").unwrap();
        let c = derive_key("другой ключ").unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), KEY_LEN);
        assert_eq!(derive_key(""), Err(CryptoError::EmptyPassword));
    }

    #[test]
    fn v2_envelope_roundtrips() {
        for text in ["фаза а", "", "эмодзи 🔐 и переносы\nстрок"] {
            let encrypted = encrypt_v2(text, &KEY).unwrap();
            assert_eq!(decrypt(&encrypted, &KEY).unwrap(), text);
        }
    }

    #[test]
    fn v2_envelope_layout() {
        let encrypted = encrypt_v2("соль", &KEY).unwrap();
        let blob = alphabet::decode(&encrypted).unwrap();
        assert_eq!(blob[0], MAGIC);
        assert_eq!(blob[1], VERSION_V2);
        assert_eq!(
            blob.len(),
            V2_HEADER_LEN + NONCE_LEN + TAG_LEN + "соль".len()
        );
        assert!(looks_encrypted(&encrypted));
    }

    #[test]
    fn v2_envelopes_use_unique_salts() {
        let a = alphabet::decode(&encrypt_v2("x", &KEY).unwrap()).unwrap();
        let b = alphabet::decode(&encrypt_v2("x", &KEY).unwrap()).unwrap();
        assert_ne!(a[HEADER_LEN..V2_HEADER_LEN], b[HEADER_LEN..V2_HEADER_LEN]);
    }

    #[test]
    fn v1_and_v2_cross_decrypt_with_one_key() {
        let key = derive_key("кросс версия").unwrap();
        let v1 = encrypt("старый формат", &key).unwrap();
        let v2 = encrypt_v2("новый формат", &key).unwrap();
        assert_eq!(decrypt(&v1, &key).unwrap(), "старый формат");
        assert_eq!(decrypt(&v2, &key).unwrap(), "новый формат");
    }

    #[test]
    fn v2_wrong_key_is_rejected() {
        let encrypted = encrypt_v2("секрет", &KEY).unwrap();
        assert_eq!(decrypt(&encrypted, &OTHER_KEY), Err(CryptoError::WrongKey));
    }

    #[test]
    fn v2_tampered_salt_is_rejected() {
        let mut blob = alphabet::decode(&encrypt_v2("секрет", &KEY).unwrap()).unwrap();
        blob[HEADER_LEN] ^= 0x01;
        let text = alphabet::encode(&blob);
        assert_eq!(decrypt(&text, &KEY), Err(CryptoError::WrongKey));
    }

    #[test]
    fn truncated_v2_envelope_is_not_encrypted() {
        let mut blob = vec![MAGIC, VERSION_V2];
        blob.extend_from_slice(&[0u8; 10]);
        let text = alphabet::encode(&blob);
        assert!(!looks_encrypted(&text));
        assert_eq!(decrypt(&text, &KEY), Err(CryptoError::NotEncrypted));
    }

    #[test]
    fn cross_version_vectors_stay_decryptable() {
        let key = derive_key("вектор два").unwrap();
        assert_eq!(decrypt("ймбк бшрже оьэмэццы эвыюфмлу довльсйт щкаь буаюб шннжмхмы ъфдмо пгвб юрча жмйюэ пофо япэцщ нйлиядй зоъкша жняст лчыщм уфуарныя фысп эюмщаоюж йюсчд", &key).unwrap(), "кросс версия тест");
        let v1 = encrypt("кросс версия тест", &key).unwrap();
        assert_eq!(decrypt(&v1, &key).unwrap(), "кросс версия тест");
    }

    #[test]
    fn unknown_envelope_version_is_rejected() {
        let mut blob = alphabet::decode(&encrypt("секрет", &KEY).unwrap()).unwrap();
        blob[1] = 0x03;
        let text = alphabet::encode(&blob);
        assert!(!looks_encrypted(&text));
        assert_eq!(decrypt(&text, &KEY), Err(CryptoError::NotEncrypted));
    }
}
