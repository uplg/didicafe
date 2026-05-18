use anyhow::Result;
use rand::RngExt;

use crate::config::TokenConfig;
use crate::db::Database;

/// Validate the format of a token code against the configured prefix, charset, and length.
///
/// Expected format: `PREFIX-XXXX-XXXX` where X is from the configured charset.
/// Returns `Ok(())` if valid, `Err(message)` describing the problem otherwise.
pub fn validate_token_format(config: &TokenConfig, code: &str) -> std::result::Result<(), String> {
    let expected_prefix = format!("{}-", config.prefix);

    if !code.starts_with(&expected_prefix) {
        return Err("Invalid token format.".to_string());
    }

    let body = &code[expected_prefix.len()..];
    let half = config.length / 2;

    // Expected body format: XXXX-XXXX (two groups of `half` chars separated by a dash)
    let expected_body_len = half + 1 + half; // e.g. 4 + 1 + 4 = 9
    if body.len() != expected_body_len {
        return Err("Invalid token format.".to_string());
    }

    // Check the dash separator
    if body.as_bytes()[half] != b'-' {
        return Err("Invalid token format.".to_string());
    }

    // Check all non-dash characters are in the allowed charset
    let charset = &config.charset;
    for (i, ch) in body.chars().enumerate() {
        if i == half {
            continue; // skip the dash
        }
        if !charset.contains(ch) {
            return Err("Invalid token format.".to_string());
        }
    }

    Ok(())
}

/// Generate a batch of tokens for a given plan.
///
/// Token format: `PREFIX-XXXX-XXXX` where X is drawn from the configured charset
/// (no 0/O/1/I to avoid human confusion).
pub async fn generate_tokens(
    db: &Database,
    config: &TokenConfig,
    plan_id: i64,
    count: usize,
    name: Option<&str>,
) -> Result<Vec<String>> {
    // Generate all codes synchronously first (ThreadRng is !Send,
    // so it must not be held across .await points).
    let codes = {
        let charset: Vec<char> = config.charset.chars().collect();
        let mut rng = rand::rng();
        (0..count)
            .map(|_| {
                let half = config.length / 2;
                let part1: String = (0..half)
                    .map(|_| charset[rng.random_range(0..charset.len())])
                    .collect();
                let part2: String = (0..half)
                    .map(|_| charset[rng.random_range(0..charset.len())])
                    .collect();
                format!("{}-{}-{}", config.prefix, part1, part2)
            })
            .collect::<Vec<_>>()
    };

    // Insert all generated codes into the database
    for code in &codes {
        db.create_token(code, name, plan_id).await?;
    }

    Ok(codes)
}

/// Result of validating a token code against the database.
#[derive(Debug)]
pub enum TokenLookup {
    /// Token is unused — ready for a fresh session.
    Unused(crate::db::Token),
    /// Token is active with remaining time — eligible for session migration
    /// (e.g. client reconnected with a different MAC after WiFi outage).
    Active(crate::db::Token),
    /// Token not found, already expired, revoked, or otherwise invalid.
    Invalid,
}

/// Validate a token code and classify it for the portal auth flow.
///
/// Returns `Unused` for fresh tokens, `Active` for tokens that still have
/// remaining session time (MAC migration case), and `Invalid` for everything else.
pub async fn validate_token(db: &Database, code: &str) -> Result<TokenLookup> {
    let token = db.get_token_by_code(code).await?;

    match token {
        Some(t) if t.status == crate::db::TokenStatus::Unused => Ok(TokenLookup::Unused(t)),
        Some(t) if t.status == crate::db::TokenStatus::Active => Ok(TokenLookup::Active(t)),
        _ => Ok(TokenLookup::Invalid),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> TokenConfig {
        TokenConfig {
            prefix: "DIDI".to_string(),
            charset: "ABCDEFGHJKLMNPQRSTUVWXYZ23456789".to_string(),
            length: 8,
        }
    }

    #[test]
    fn test_valid_token_format() {
        let cfg = test_config();
        assert!(validate_token_format(&cfg, "DIDI-ABCD-EF23").is_ok());
        assert!(validate_token_format(&cfg, "DIDI-2345-6789").is_ok());
        assert!(validate_token_format(&cfg, "DIDI-WXYZ-HKLM").is_ok());
    }

    #[test]
    fn test_wrong_prefix() {
        let cfg = test_config();
        assert!(validate_token_format(&cfg, "CAFE-ABCD-EF23").is_err());
        assert!(validate_token_format(&cfg, "ABCD-EF23").is_err());
        assert!(validate_token_format(&cfg, "abcd-ef23").is_err());
    }

    #[test]
    fn test_wrong_length() {
        let cfg = test_config();
        // Too short
        assert!(validate_token_format(&cfg, "DIDI-ABC-EF23").is_err());
        // Too long
        assert!(validate_token_format(&cfg, "DIDI-ABCDE-EF23").is_err());
        // Missing second group
        assert!(validate_token_format(&cfg, "DIDI-ABCD").is_err());
    }

    #[test]
    fn test_missing_dash_separator() {
        let cfg = test_config();
        assert!(validate_token_format(&cfg, "DIDI-ABCDEF23").is_err());
    }

    #[test]
    fn test_invalid_charset_chars() {
        let cfg = test_config();
        // 0, O, 1, I are excluded from charset
        assert!(validate_token_format(&cfg, "DIDI-ABCO-EF23").is_err());
        assert!(validate_token_format(&cfg, "DIDI-ABC0-EF23").is_err());
        assert!(validate_token_format(&cfg, "DIDI-ABC1-EF23").is_err());
        assert!(validate_token_format(&cfg, "DIDI-ABCI-EF23").is_err());
        // Lowercase not in charset
        assert!(validate_token_format(&cfg, "DIDI-abcd-ef23").is_err());
    }

    #[test]
    fn test_empty_and_garbage_input() {
        let cfg = test_config();
        assert!(validate_token_format(&cfg, "").is_err());
        assert!(validate_token_format(&cfg, "not-a-token").is_err());
        assert!(validate_token_format(&cfg, "DIDI-").is_err());
        assert!(validate_token_format(&cfg, "DIDI-????-!!!!").is_err());
    }

    #[test]
    fn test_generated_tokens_pass_validation() {
        // Ensure tokens produced by generate_tokens would pass format validation
        let cfg = test_config();
        let charset: Vec<char> = cfg.charset.chars().collect();
        let mut rng = rand::rng();
        for _ in 0..100 {
            let half = cfg.length / 2;
            let part1: String = (0..half)
                .map(|_| charset[rng.random_range(0..charset.len())])
                .collect();
            let part2: String = (0..half)
                .map(|_| charset[rng.random_range(0..charset.len())])
                .collect();
            let code = format!("{}-{}-{}", cfg.prefix, part1, part2);
            assert!(
                validate_token_format(&cfg, &code).is_ok(),
                "generated token failed validation: {code}"
            );
        }
    }
}
