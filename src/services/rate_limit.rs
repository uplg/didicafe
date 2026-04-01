use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::RwLock;
use std::time::Instant;

use crate::config::RateLimitConfig;

/// Per-IP attempt tracking for token auth rate limiting.
///
/// Two-tier defense:
/// 1. **Windowed throttle**: After `max_auth_attempts` within `auth_window_seconds`,
///    subsequent attempts are rejected with 429.
/// 2. **Ban**: After `ban_after_attempts` total within the window, the IP is banned
///    for `ban_duration_seconds`. During ban, all attempts are rejected regardless
///    of the window state.
///
/// This prevents brute-force token guessing while keeping the portal responsive
/// for legitimate users. Never leaks whether a token exists (OWASP).
pub struct RateLimiter {
    /// Per-IP attempt timestamps within the current window.
    attempts: RwLock<HashMap<IpAddr, Vec<Instant>>>,
    /// Per-IP ban expiry. If present and in the future, all attempts are rejected.
    bans: RwLock<HashMap<IpAddr, Instant>>,
    config: RateLimitConfig,
}

/// Result of checking rate limits for a given IP.
#[derive(Debug, PartialEq, Eq)]
pub enum RateLimitResult {
    /// Request is allowed.
    Allowed,
    /// Request is throttled (too many attempts in window).
    Throttled,
    /// IP is banned (exceeded ban threshold).
    Banned {
        /// Seconds remaining on the ban.
        retry_after_seconds: u64,
    },
}

impl RateLimiter {
    pub fn new(config: &RateLimitConfig) -> Self {
        Self {
            attempts: RwLock::new(HashMap::new()),
            bans: RwLock::new(HashMap::new()),
            config: config.clone(),
        }
    }

    /// Check whether a request from `ip` should be allowed, and record the attempt.
    ///
    /// Call this *before* processing the auth request. If the result is not `Allowed`,
    /// the handler should return 429 immediately without touching the database.
    pub fn check_and_record(&self, ip: IpAddr) -> RateLimitResult {
        let now = Instant::now();

        // 1. Check if IP is currently banned
        {
            let bans = self.bans.read().expect("bans lock poisoned");
            if let Some(&ban_until) = bans.get(&ip)
                && now < ban_until
            {
                let remaining = (ban_until - now).as_secs();
                return RateLimitResult::Banned {
                    retry_after_seconds: remaining.max(1),
                };
            }
        }

        // 2. Remove expired bans
        {
            let mut bans = self.bans.write().expect("bans lock poisoned");
            bans.retain(|_, ban_until| now < *ban_until);
        }

        // 3. Record attempt and check thresholds
        let window = std::time::Duration::from_secs(self.config.auth_window_seconds);
        let mut attempts = self.attempts.write().expect("attempts lock poisoned");

        // Periodic sweep: prune stale IPs to prevent unbounded memory growth.
        // Run every 256 calls (cheap check via entry count modulo).
        if !attempts.is_empty() && attempts.len().is_multiple_of(256) {
            attempts.retain(|_, timestamps| {
                timestamps.retain(|&ts| now.duration_since(ts) < window);
                !timestamps.is_empty()
            });
        }

        let entry = attempts.entry(ip).or_default();

        // Prune attempts outside the current window
        entry.retain(|&ts| now.duration_since(ts) < window);

        // Record this attempt
        entry.push(now);

        let count = entry.len() as u32;

        // 4. Check ban threshold first (higher severity)
        if count >= self.config.ban_after_attempts {
            // Ban this IP
            let ban_until = now + std::time::Duration::from_secs(self.config.ban_duration_seconds);
            self.bans
                .write()
                .expect("bans lock poisoned")
                .insert(ip, ban_until);
            // Clear attempts (ban takes over)
            entry.clear();

            return RateLimitResult::Banned {
                retry_after_seconds: self.config.ban_duration_seconds,
            };
        }

        // 5. Check throttle threshold
        if count > self.config.max_auth_attempts {
            return RateLimitResult::Throttled;
        }

        RateLimitResult::Allowed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> RateLimitConfig {
        RateLimitConfig {
            max_auth_attempts: 3,
            auth_window_seconds: 60,
            ban_after_attempts: 5,
            ban_duration_seconds: 900,
        }
    }

    #[test]
    fn test_allows_under_threshold() {
        let limiter = RateLimiter::new(&test_config());
        let ip: IpAddr = "192.168.1.10".parse().unwrap();

        // First 3 attempts should be allowed
        assert_eq!(limiter.check_and_record(ip), RateLimitResult::Allowed);
        assert_eq!(limiter.check_and_record(ip), RateLimitResult::Allowed);
        assert_eq!(limiter.check_and_record(ip), RateLimitResult::Allowed);
    }

    #[test]
    fn test_throttles_after_max_attempts() {
        let limiter = RateLimiter::new(&test_config());
        let ip: IpAddr = "192.168.1.10".parse().unwrap();

        // Use up the allowed attempts
        for _ in 0..3 {
            assert_eq!(limiter.check_and_record(ip), RateLimitResult::Allowed);
        }

        // 4th attempt should be throttled
        assert_eq!(limiter.check_and_record(ip), RateLimitResult::Throttled);
    }

    #[test]
    fn test_bans_after_ban_threshold() {
        let limiter = RateLimiter::new(&test_config());
        let ip: IpAddr = "192.168.1.10".parse().unwrap();

        // 3 allowed + 1 throttled = 4 attempts, then 5th triggers ban
        for _ in 0..3 {
            limiter.check_and_record(ip);
        }
        assert_eq!(limiter.check_and_record(ip), RateLimitResult::Throttled);
        assert!(matches!(
            limiter.check_and_record(ip),
            RateLimitResult::Banned { .. }
        ));
    }

    #[test]
    fn test_banned_ip_stays_banned() {
        let limiter = RateLimiter::new(&test_config());
        let ip: IpAddr = "192.168.1.10".parse().unwrap();

        // Trigger ban
        for _ in 0..5 {
            limiter.check_and_record(ip);
        }

        // Subsequent attempts are still banned
        assert!(matches!(
            limiter.check_and_record(ip),
            RateLimitResult::Banned { .. }
        ));
        assert!(matches!(
            limiter.check_and_record(ip),
            RateLimitResult::Banned { .. }
        ));
    }

    #[test]
    fn test_different_ips_independent() {
        let limiter = RateLimiter::new(&test_config());
        let ip1: IpAddr = "192.168.1.10".parse().unwrap();
        let ip2: IpAddr = "192.168.1.11".parse().unwrap();

        // Exhaust ip1's allowed attempts
        for _ in 0..3 {
            limiter.check_and_record(ip1);
        }
        assert_eq!(limiter.check_and_record(ip1), RateLimitResult::Throttled);

        // ip2 should still be allowed
        assert_eq!(limiter.check_and_record(ip2), RateLimitResult::Allowed);
    }

    #[test]
    fn test_ban_has_retry_after() {
        let limiter = RateLimiter::new(&test_config());
        let ip: IpAddr = "192.168.1.10".parse().unwrap();

        for _ in 0..5 {
            limiter.check_and_record(ip);
        }

        match limiter.check_and_record(ip) {
            RateLimitResult::Banned {
                retry_after_seconds,
            } => {
                // Should be close to ban_duration_seconds (900)
                assert!(retry_after_seconds > 0);
                assert!(retry_after_seconds <= 900);
            }
            other => panic!("expected Banned, got {other:?}"),
        }
    }

    #[test]
    fn test_window_expiry() {
        // Use a 0-second window so all attempts expire immediately
        let config = RateLimitConfig {
            max_auth_attempts: 2,
            auth_window_seconds: 0,
            ban_after_attempts: 5,
            ban_duration_seconds: 900,
        };
        let limiter = RateLimiter::new(&config);
        let ip: IpAddr = "192.168.1.10".parse().unwrap();

        // With a 0-second window, previous attempts are pruned immediately.
        // Sleep briefly so the prune logic catches them.
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert_eq!(limiter.check_and_record(ip), RateLimitResult::Allowed);
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert_eq!(limiter.check_and_record(ip), RateLimitResult::Allowed);
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert_eq!(limiter.check_and_record(ip), RateLimitResult::Allowed);
    }
}
