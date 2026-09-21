use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct EtagCache {
    entries: HashMap<String, (String, String)>,
}

impl EtagCache {
    pub(crate) fn get(&self, url: &str) -> Option<&(String, String)> {
        self.entries.get(url)
    }

    pub(crate) fn store(&mut self, url: &str, etag: String, body: String) {
        self.entries.insert(url.to_string(), (etag, body));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stores_and_returns_entries_per_url() {
        let mut cache = EtagCache::default();
        cache.store("https://api/a", "\"e1\"".to_string(), "body-a".to_string());
        assert_eq!(
            cache.get("https://api/a"),
            Some(&("\"e1\"".to_string(), "body-a".to_string()))
        );
        assert_eq!(cache.get("https://api/b"), None);
    }

    #[test]
    fn storing_again_overwrites() {
        let mut cache = EtagCache::default();
        cache.store("u", "\"1\"".to_string(), "old".to_string());
        cache.store("u", "\"2\"".to_string(), "new".to_string());
        assert_eq!(
            cache.get("u"),
            Some(&("\"2\"".to_string(), "new".to_string()))
        );
    }
}
