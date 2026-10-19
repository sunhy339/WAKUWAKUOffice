use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use sha2::{Digest, Sha256};

/// 生成只用于一次密码重置的不可猜测 Token。
pub fn generate_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// 数据库只保存 Token 摘要，避免数据库泄露后可以直接重置密码。
pub fn hash_token(token: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(token.as_bytes()))
}

pub fn build_reset_url(base_url: &str, token: &str) -> String {
    format!(
        "{}?token={}",
        base_url.trim_end_matches('/'),
        urlencoding::encode(token)
    )
}

#[cfg(test)]
mod tests {
    use super::{build_reset_url, generate_token, hash_token};

    #[test]
    fn token_is_opaque_and_hash_is_stable() {
        let token = generate_token();
        assert_eq!(token.len(), 64);
        assert_eq!(hash_token(&token), hash_token(&token));
        assert_ne!(token, generate_token());
    }

    #[test]
    fn reset_url_contains_encoded_token() {
        let url = build_reset_url("http://localhost:5173/reset-password/", "abc+123");
        assert_eq!(url, "http://localhost:5173/reset-password?token=abc%2B123");
    }
}
