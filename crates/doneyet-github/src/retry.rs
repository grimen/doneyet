use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 2,
            base_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(5),
        }
    }
}

impl RetryPolicy {
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }

    pub fn delay_for(&self, attempt: u32) -> Duration {
        (self.base_delay * attempt.max(1)).min(self.max_delay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_up_to_max() {
        let policy = RetryPolicy {
            max_retries: 2,
            ..RetryPolicy::default()
        };
        assert!(policy.should_retry(0));
        assert!(policy.should_retry(1));
        assert!(!policy.should_retry(2));
        assert!(!policy.should_retry(9));
    }

    #[test]
    fn zero_retries_never_retries() {
        let policy = RetryPolicy {
            max_retries: 0,
            ..RetryPolicy::default()
        };
        assert!(!policy.should_retry(0));
    }

    #[test]
    fn delay_scales_linearly_with_attempt() {
        let policy = RetryPolicy {
            base_delay: Duration::from_millis(100),
            ..RetryPolicy::default()
        };
        assert_eq!(policy.delay_for(1), Duration::from_millis(100));
        assert_eq!(policy.delay_for(2), Duration::from_millis(200));
        assert_eq!(policy.delay_for(3), Duration::from_millis(300));
    }

    #[test]
    fn delay_for_attempt_is_capped_at_max_delay() {
        let policy = RetryPolicy {
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(5),
            ..RetryPolicy::default()
        };
        assert_eq!(policy.delay_for(1), Duration::from_secs(1));
        assert_eq!(policy.delay_for(5), Duration::from_secs(5));
        assert_eq!(policy.delay_for(10), Duration::from_secs(5));
    }
}
