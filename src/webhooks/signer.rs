use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

/// Sign a webhook payload using HMAC-SHA256
pub fn sign_payload(payload: &[u8], secret: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(payload);
    let result = mac.finalize();
    hex::encode(result.into_bytes())
}

/// Verify a webhook signature
pub fn verify_signature(payload: &[u8], secret: &str, signature: &str) -> bool {
    let expected = sign_payload(payload, secret);
    // Use constant-time comparison to prevent timing attacks
    constant_time_eq(expected.as_bytes(), signature.as_bytes())
}

/// Constant-time string comparison
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    let mut result = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify() {
        let payload = b"test payload";
        let secret = "my-secret-key";
        
        let signature = sign_payload(payload, secret);
        assert!(verify_signature(payload, secret, &signature));
        assert!(!verify_signature(payload, "wrong-secret", &signature));
        assert!(!verify_signature(b"wrong payload", secret, &signature));
    }
}
