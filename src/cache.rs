use moka::future::Cache;
use std::sync::Arc;
use std::time::Duration;

/// Source type indicator for cached content
#[derive(Clone, Debug, PartialEq)]
pub enum ContentSource {
    /// Content was fetched from a native markdown source
    Native,
    /// Content was converted from HTML
    Converted,
}

impl ContentSource {
    pub fn as_header_value(&self) -> &'static str {
        match self {
            ContentSource::Native => "native",
            ContentSource::Converted => "converted",
        }
    }
}

/// Cached markdown content with metadata
#[derive(Clone)]
pub struct CachedContent {
    pub markdown: String,
    pub source: ContentSource,
}

/// Cache wrapper for markdown content
#[derive(Clone)]
pub struct MarkdownCache {
    cache: Arc<Cache<String, CachedContent>>,
}

impl MarkdownCache {
    /// Create a new cache with the specified TTL
    pub fn new(ttl_secs: u64) -> Self {
        let cache = Cache::builder()
            .time_to_live(Duration::from_secs(ttl_secs))
            .max_capacity(10_000) // Max 10k entries
            .build();

        Self {
            cache: Arc::new(cache),
        }
    }

    /// Get cached content for a URL
    pub async fn get(&self, url: &str) -> Option<CachedContent> {
        self.cache.get(&normalize_cache_key(url)).await
    }

    /// Store content in cache
    pub async fn set(&self, url: &str, markdown: String, source: ContentSource) {
        let content = CachedContent { markdown, source };
        self.cache.insert(normalize_cache_key(url), content).await;
    }

}

/// Normalize URL for cache key
/// - Lowercase the host
/// - Remove fragment
/// - Keep query string as-is (order matters for some APIs)
fn normalize_cache_key(url: &str) -> String {
    // Simple normalization: just lowercase and remove fragment
    let url = url.split('#').next().unwrap_or(url);
    url.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_set_get() {
        let cache = MarkdownCache::new(3600);

        cache
            .set(
                "https://example.com/article",
                "# Hello".to_string(),
                ContentSource::Native,
            )
            .await;

        let result = cache.get("https://example.com/article").await;
        assert!(result.is_some());

        let content = result.unwrap();
        assert_eq!(content.markdown, "# Hello");
        assert_eq!(content.source, ContentSource::Native);
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let cache = MarkdownCache::new(3600);
        let result = cache.get("https://example.com/nonexistent").await;
        assert!(result.is_none());
    }

    #[test]
    fn test_normalize_cache_key() {
        // Should lowercase
        assert_eq!(
            normalize_cache_key("HTTPS://EXAMPLE.COM/Path"),
            "https://example.com/path"
        );

        // Should remove fragment
        assert_eq!(
            normalize_cache_key("https://example.com/page#section"),
            "https://example.com/page"
        );

        // Should keep query string
        assert_eq!(
            normalize_cache_key("https://example.com/page?a=1&b=2"),
            "https://example.com/page?a=1&b=2"
        );
    }

    #[test]
    fn test_content_source_header() {
        assert_eq!(ContentSource::Native.as_header_value(), "native");
        assert_eq!(ContentSource::Converted.as_header_value(), "converted");
    }
}
