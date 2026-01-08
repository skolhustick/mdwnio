use crate::error::{MdwnError, Result};
use futures_util::StreamExt;
use ipnetwork::IpNetwork;
use reqwest::Client;
use std::net::IpAddr;
use std::sync::LazyLock;
use std::time::Duration;
use url::Url;

/// Private/internal IP ranges that should be blocked (SSRF protection)
static BLOCKED_NETWORKS: LazyLock<Vec<IpNetwork>> = LazyLock::new(|| {
    vec![
        // IPv4 private ranges
        "10.0.0.0/8".parse().expect("valid CIDR"),
        "172.16.0.0/12".parse().expect("valid CIDR"),
        "192.168.0.0/16".parse().expect("valid CIDR"),
        // Loopback
        "127.0.0.0/8".parse().expect("valid CIDR"),
        // Link-local
        "169.254.0.0/16".parse().expect("valid CIDR"),
        // AWS metadata endpoint
        "169.254.169.254/32".parse().expect("valid CIDR"),
        // Broadcast
        "255.255.255.255/32".parse().expect("valid CIDR"),
        // Current network
        "0.0.0.0/8".parse().expect("valid CIDR"),
        // IPv6 loopback
        "::1/128".parse().expect("valid CIDR"),
        // IPv6 link-local
        "fe80::/10".parse().expect("valid CIDR"),
        // IPv6 unique local
        "fc00::/7".parse().expect("valid CIDR"),
    ]
});

/// Configuration for the HTTP client
#[derive(Clone)]
pub struct FetchConfig {
    pub user_agent: String,
    pub timeout_secs: u64,
    pub max_content_length: usize,
    pub max_redirects: usize,
}

impl Default for FetchConfig {
    fn default() -> Self {
        Self {
            user_agent: "mdwn.io/1.0 (+https://mdwn.io)".to_string(),
            timeout_secs: 10,
            max_content_length: 10 * 1024 * 1024, // 10MB
            max_redirects: 5,
        }
    }
}

/// HTTP client wrapper with SSRF protection
pub struct Fetcher {
    client: Client,
    config: FetchConfig,
}

impl Fetcher {
    /// Create a new Fetcher with the given configuration
    pub fn new(config: FetchConfig) -> Result<Self> {
        let client = Client::builder()
            .user_agent(&config.user_agent)
            .timeout(Duration::from_secs(config.timeout_secs))
            .redirect(reqwest::redirect::Policy::none()) // Handle redirects manually for SSRF protection
            .gzip(true)
            .brotli(true)
            .deflate(true)
            .build()
            .map_err(|e| MdwnError::Internal(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self { client, config })
    }

    /// Validate and parse a URL from the request path
    pub fn parse_url(&self, url_str: &str) -> Result<Url> {
        // First, try to parse as-is to detect non-http schemes
        if let Ok(url) = Url::parse(url_str) {
            // Check scheme before doing anything else
            match url.scheme() {
                "http" | "https" => {
                    // Continue with validation below
                }
                scheme => {
                    return Err(MdwnError::InvalidUrl(format!(
                        "Scheme '{}' not allowed, only http/https",
                        scheme
                    )))
                }
            }

            // Reject URLs with credentials
            if url.username() != "" || url.password().is_some() {
                return Err(MdwnError::InvalidUrl(
                    "URLs with credentials are not allowed".to_string(),
                ));
            }

            // Validate host exists
            if url.host_str().is_none() {
                return Err(MdwnError::InvalidUrl("URL must have a host".to_string()));
            }

            return Ok(url);
        }

        // If parsing failed, try prepending https://
        let url_with_scheme = format!("https://{}", url_str);
        let url = Url::parse(&url_with_scheme).map_err(|e| MdwnError::InvalidUrl(e.to_string()))?;

        // Reject URLs with credentials
        if url.username() != "" || url.password().is_some() {
            return Err(MdwnError::InvalidUrl(
                "URLs with credentials are not allowed".to_string(),
            ));
        }

        // Validate host exists
        if url.host_str().is_none() {
            return Err(MdwnError::InvalidUrl("URL must have a host".to_string()));
        }

        Ok(url)
    }

    /// Check if an IP address is blocked (private/internal)
    fn is_blocked_ip(&self, ip: IpAddr) -> bool {
        BLOCKED_NETWORKS.iter().any(|network| network.contains(ip))
    }

    /// Resolve hostname and check if the IP is blocked
    async fn check_ssrf(&self, url: &Url) -> Result<()> {
        let host = url.host_str().ok_or_else(|| MdwnError::InvalidUrl("No host".to_string()))?;

        // Try to parse as IP directly
        if let Ok(ip) = host.parse::<IpAddr>() {
            if self.is_blocked_ip(ip) {
                return Err(MdwnError::BlockedUrl);
            }
            return Ok(());
        }

        // Resolve hostname to IPs
        let port = url.port_or_known_default().unwrap_or(443);
        let addrs = tokio::net::lookup_host(format!("{}:{}", host, port))
            .await
            .map_err(|e| MdwnError::FetchFailed(format!("DNS resolution failed: {}", e)))?;

        for addr in addrs {
            if self.is_blocked_ip(addr.ip()) {
                return Err(MdwnError::BlockedUrl);
            }
        }

        Ok(())
    }

    /// Fetch a URL with SSRF protection
    pub async fn fetch(&self, url: &Url) -> Result<FetchResponse> {
        self.fetch_with_redirects(url, 0).await
    }

    /// Internal fetch with redirect tracking
    fn fetch_with_redirects<'a>(
        &'a self,
        url: &'a Url,
        redirect_count: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<FetchResponse>> + Send + 'a>> {
        Box::pin(async move {
            if redirect_count > self.config.max_redirects {
                return Err(MdwnError::FetchFailed(format!(
                    "Too many redirects (max {})",
                    self.config.max_redirects
                )));
            }

            // Check SSRF before every request (including redirects)
            self.check_ssrf(url).await?;

            let response = self
                .client
                .get(url.as_str())
                .send()
                .await
                .map_err(|e| {
                    if e.is_timeout() {
                        MdwnError::Timeout(self.config.timeout_secs)
                    } else {
                        MdwnError::FetchFailed(e.to_string())
                    }
                })?;

            // Handle redirects manually to re-check SSRF
            if response.status().is_redirection()
                && let Some(location) = response.headers().get("location") {
                    let location_str = location
                        .to_str()
                        .map_err(|_| MdwnError::FetchFailed("Invalid redirect location".to_string()))?;

                    // Resolve relative redirects
                    let redirect_url = url
                        .join(location_str)
                        .map_err(|e| MdwnError::FetchFailed(format!("Invalid redirect URL: {}", e)))?;

                    // Validate the redirect URL
                    let redirect_url = self.parse_url(redirect_url.as_str())?;

                    return self.fetch_with_redirects(&redirect_url, redirect_count + 1).await;
                }

            // Check content length before reading body
            if let Some(content_length) = response.content_length()
                && content_length as usize > self.config.max_content_length {
                    return Err(MdwnError::TooLarge(self.config.max_content_length));
                }

            // Check status code
            let status = response.status();
            if status == reqwest::StatusCode::NOT_FOUND {
                return Err(MdwnError::NotFound);
            }
            if status == reqwest::StatusCode::FORBIDDEN {
                return Err(MdwnError::Forbidden);
            }
            if !status.is_success() {
                return Err(MdwnError::FetchFailed(format!(
                    "Upstream returned status {}",
                    status.as_u16()
                )));
            }

            // Extract content type
            let content_type = response
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            // Read body with size limit
            let bytes = self.read_body_limited(response).await?;

            Ok(FetchResponse {
                content_type,
                body: bytes,
                final_url: url.clone(),
            })
        })
    }

    /// Read response body with size limit
    async fn read_body_limited(&self, response: reqwest::Response) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut stream = response.bytes_stream();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| MdwnError::FetchFailed(format!("Read error: {}", e)))?;
            bytes.extend_from_slice(&chunk);

            if bytes.len() > self.config.max_content_length {
                return Err(MdwnError::TooLarge(self.config.max_content_length));
            }
        }

        Ok(bytes)
    }
}

/// Response from a fetch operation
pub struct FetchResponse {
    pub content_type: Option<String>,
    pub body: Vec<u8>,
    pub final_url: Url,
}

impl FetchResponse {
    /// Get the primary MIME type (without charset or other parameters)
    pub fn mime_type(&self) -> Option<&str> {
        self.content_type.as_ref().map(|ct| {
            ct.split(';').next().unwrap_or(ct).trim()
        })
    }

    /// Decode body as UTF-8 (with fallback for invalid sequences)
    pub fn body_as_string(&self) -> String {
        // Try to decode as UTF-8, replacing invalid sequences
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url_valid() {
        let fetcher = Fetcher::new(FetchConfig::default()).unwrap();
        assert!(fetcher.parse_url("https://example.com").is_ok());
        assert!(fetcher.parse_url("http://example.com/path").is_ok());
        assert!(fetcher.parse_url("example.com/path").is_ok()); // Should prepend https://
    }

    #[test]
    fn test_parse_url_blocked_schemes() {
        let fetcher = Fetcher::new(FetchConfig::default()).unwrap();
        // file:// URLs should be rejected
        let result = fetcher.parse_url("file:///etc/passwd");
        assert!(result.is_err(), "file:// scheme should be blocked: {:?}", result);

        // ftp:// URLs should be rejected
        let result = fetcher.parse_url("ftp://example.com");
        assert!(result.is_err(), "ftp:// scheme should be blocked: {:?}", result);

        // gopher:// URLs should be rejected
        let result = fetcher.parse_url("gopher://example.com");
        assert!(result.is_err(), "gopher:// scheme should be blocked: {:?}", result);
    }

    #[test]
    fn test_parse_url_with_credentials() {
        let fetcher = Fetcher::new(FetchConfig::default()).unwrap();
        assert!(fetcher.parse_url("https://user:pass@example.com").is_err());
    }

    #[test]
    fn test_blocked_ips() {
        let fetcher = Fetcher::new(FetchConfig::default()).unwrap();

        // Private IPs should be blocked
        assert!(fetcher.is_blocked_ip("127.0.0.1".parse().unwrap()));
        assert!(fetcher.is_blocked_ip("10.0.0.1".parse().unwrap()));
        assert!(fetcher.is_blocked_ip("192.168.1.1".parse().unwrap()));
        assert!(fetcher.is_blocked_ip("172.16.0.1".parse().unwrap()));
        assert!(fetcher.is_blocked_ip("169.254.169.254".parse().unwrap())); // AWS metadata

        // Public IPs should not be blocked
        assert!(!fetcher.is_blocked_ip("8.8.8.8".parse().unwrap()));
        assert!(!fetcher.is_blocked_ip("1.1.1.1".parse().unwrap()));
    }

    #[test]
    fn test_mime_type_extraction() {
        let response = FetchResponse {
            content_type: Some("text/html; charset=utf-8".to_string()),
            body: vec![],
            final_url: Url::parse("https://example.com").unwrap(),
        };
        assert_eq!(response.mime_type(), Some("text/html"));
    }
}
