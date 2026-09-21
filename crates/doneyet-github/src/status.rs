use doneyet_core::ports::ProviderError;
use reqwest::Response;
use std::time::Duration;

pub(crate) fn is_rate_limited(remaining: Option<&str>) -> bool {
    remaining == Some("0")
}

pub(crate) fn retry_after_hint(
    retry_after: Option<&str>,
    reset_epoch: Option<&str>,
    now_secs: u64,
) -> Duration {
    if let Some(secs) = retry_after.and_then(|value| value.parse::<u64>().ok()) {
        return Duration::from_secs(secs);
    }
    if let Some(reset) = reset_epoch.and_then(|value| value.parse::<u64>().ok()) {
        return Duration::from_secs(reset.saturating_sub(now_secs));
    }
    Duration::from_secs(60)
}

pub(crate) fn map_forbidden(
    rate_limited: bool,
    retry_after: Option<&str>,
    reset_epoch: Option<&str>,
    now_secs: u64,
    message: String,
) -> ProviderError {
    if rate_limited {
        ProviderError::RateLimited {
            retry_after: retry_after_hint(retry_after, reset_epoch, now_secs),
        }
    } else {
        ProviderError::Other(message)
    }
}

pub(crate) async fn error_message(response: Response) -> String {
    let status = response.status();
    match response.json::<serde_json::Value>().await {
        Ok(value) => value
            .get("message")
            .and_then(|m| m.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| status.to_string()),
        Err(_) => status.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limited_only_on_zero_remaining() {
        assert!(is_rate_limited(Some("0")));
        assert!(!is_rate_limited(Some("1")));
        assert!(!is_rate_limited(None));
        assert!(!is_rate_limited(Some("garbage")));
    }

    #[test]
    fn retry_after_header_wins() {
        assert_eq!(
            retry_after_hint(Some("42"), Some("9999"), 0),
            Duration::from_secs(42)
        );
    }

    #[test]
    fn invalid_retry_after_falls_back_to_reset_epoch() {
        assert_eq!(
            retry_after_hint(Some("soon"), Some("500"), 100),
            Duration::from_secs(400)
        );
    }

    #[test]
    fn missing_everything_defaults_to_a_minute() {
        assert_eq!(retry_after_hint(None, None, 0), Duration::from_secs(60));
    }

    #[test]
    fn forbidden_maps_to_rate_limited_or_other() {
        let limited = map_forbidden(true, Some("7"), None, 0, "nope".to_string());
        assert!(matches!(
            limited,
            ProviderError::RateLimited { retry_after } if retry_after == Duration::from_secs(7)
        ));
        let plain = map_forbidden(false, None, None, 0, "forbidden".to_string());
        assert!(matches!(plain, ProviderError::Other(message) if message == "forbidden"));
    }
}
