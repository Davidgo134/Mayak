//! Anti-spoof fingerprint for the auth flow (`mode` in authRequest,
//! `chatCacheFingerprint` in login).
//!
//! the three digests (APK signature/dex/native-lib hashes) come from the caller,
//! not baked in, so they can change per app version or flavor.

use sha2::{Digest, Sha256};

/// 96-byte fingerprint: three SHA-256 of `digest || int64_be(calls_seed) ||
/// utf8(device_id)`, concatenated in signature/dex/so order.
pub fn chat_cache_fingerprint(
    signature_digest: &[u8],
    dex_digest: &[u8],
    so_digest: &[u8],
    calls_seed: i64,
    device_id: &str,
) -> Vec<u8> {
    let seed = calls_seed.to_be_bytes();
    let device = device_id.as_bytes();
    let mut out = Vec::with_capacity(96);
    out.extend_from_slice(&hash(signature_digest, &seed, device));
    out.extend_from_slice(&hash(dex_digest, &seed, device));
    out.extend_from_slice(&hash(so_digest, &seed, device));
    out
}

fn hash(prefix: &[u8], seed: &[u8], device: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    hasher.update(seed);
    hasher.update(device);
    hasher.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::chat_cache_fingerprint;

    #[test]
    fn fingerprint_is_96_bytes_and_deterministic() {
        let (sig, dex, so) = ([1u8; 32], [2u8; 32], [3u8; 32]);
        let a = chat_cache_fingerprint(&sig, &dex, &so, 12345, "dev-abc");
        let b = chat_cache_fingerprint(&sig, &dex, &so, 12345, "dev-abc");
        assert_eq!(a.len(), 96);
        assert_eq!(a, b);
    }

    #[test]
    fn fingerprint_varies_with_inputs() {
        let (sig, dex, so) = ([1u8; 32], [2u8; 32], [3u8; 32]);
        let base = chat_cache_fingerprint(&sig, &dex, &so, 1, "dev");
        assert_ne!(base, chat_cache_fingerprint(&sig, &dex, &so, 2, "dev"));
        assert_ne!(base, chat_cache_fingerprint(&sig, &dex, &so, 1, "dev2"));
        assert_ne!(
            base,
            chat_cache_fingerprint(&[9u8; 32], &dex, &so, 1, "dev")
        );
    }
}
