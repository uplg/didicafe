use anyhow::Result;

/// Validate MAC address format.
///
/// Accepts only lowercase hex-colon format: `xx:xx:xx:xx:xx:xx` (exactly 17 chars).
/// Rejects null (00:00:00:00:00:00) and broadcast (ff:ff:ff:ff:ff:ff) MACs.
///
/// Used by both the web extractor (request validation) and the firewall
/// controller (defense-in-depth before nft command interpolation).
pub fn validate_mac(mac: &str) -> Result<()> {
    let bytes = mac.as_bytes();
    if bytes.len() != 17 {
        anyhow::bail!("invalid MAC length: {}", mac.len());
    }
    for (i, &b) in bytes.iter().enumerate() {
        if i % 3 == 2 {
            if b != b':' {
                anyhow::bail!("invalid MAC separator at position {i}");
            }
        } else if !b.is_ascii_hexdigit() || (b.is_ascii_alphabetic() && !b.is_ascii_lowercase()) {
            anyhow::bail!("invalid MAC character at position {i}");
        }
    }
    if mac == "00:00:00:00:00:00" {
        anyhow::bail!("null MAC address rejected");
    }
    if mac == "ff:ff:ff:ff:ff:ff" {
        anyhow::bail!("broadcast MAC address rejected");
    }
    Ok(())
}

/// Check whether a MAC string is valid (non-error version for extractors).
pub fn is_valid_mac(mac: &str) -> bool {
    validate_mac(mac).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_macs() {
        assert!(validate_mac("aa:bb:cc:dd:ee:ff").is_ok());
        assert!(validate_mac("02:00:00:00:00:01").is_ok());
        assert!(validate_mac("12:34:56:78:9a:bc").is_ok());
    }

    #[test]
    fn test_rejects_uppercase() {
        assert!(validate_mac("AA:BB:CC:DD:EE:FF").is_err());
    }

    #[test]
    fn test_rejects_null() {
        assert!(validate_mac("00:00:00:00:00:00").is_err());
    }

    #[test]
    fn test_rejects_broadcast() {
        assert!(validate_mac("ff:ff:ff:ff:ff:ff").is_err());
    }

    #[test]
    fn test_rejects_injection() {
        assert!(
            validate_mac("aa:bb:cc:dd:ee:ff } ; add rule inet didicafe input accept ; #").is_err()
        );
        assert!(validate_mac("aa:bb:cc:dd:ee:f").is_err());
        assert!(validate_mac("not-a-mac").is_err());
        assert!(validate_mac("").is_err());
    }

    #[test]
    fn test_rejects_bad_formats() {
        assert!(!is_valid_mac("aa:bb:cc:dd:ee")); // too short
        assert!(!is_valid_mac("aa:bb:cc:dd:ee:ff:00")); // too long
        assert!(!is_valid_mac("aa-bb-cc-dd-ee-ff")); // dashes
        assert!(!is_valid_mac("not-a-mac"));
        assert!(!is_valid_mac(""));
    }
}
