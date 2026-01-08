mod cache;
mod convert;
mod error;
mod fetch;
mod parse;

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use cache::{ContentSource, MarkdownCache};
use error::{MdwnError, Result};
use fetch::{FetchConfig, Fetcher};
use parse::{categorize_content_type, parse_html_for_markdown_link, parse_json_for_markdown};
use parse::{ContentCategory, HtmlParseResult, JsonParseResult};
use std::env;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::info;
use tracing_subscriber::EnvFilter;

/// Application configuration
#[derive(Clone)]
struct Config {
    port: u16,
    cache_ttl: u64,
    request_timeout: u64,
    max_content_length: usize,
    max_redirects: usize,
    user_agent: String,
}

impl Config {
    fn from_env() -> Self {
        Self {
            port: env::var("PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3000),
            cache_ttl: env::var("CACHE_TTL")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3600),
            request_timeout: env::var("REQUEST_TIMEOUT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10),
            max_content_length: env::var("MAX_CONTENT_LENGTH")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(10 * 1024 * 1024), // 10MB
            max_redirects: env::var("MAX_REDIRECTS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            user_agent: env::var("USER_AGENT")
                .unwrap_or_else(|_| "mdwn.io/1.0 (+https://mdwn.io)".to_string()),
        }
    }
}

/// Shared application state
#[derive(Clone)]
struct AppState {
    fetcher: Arc<Fetcher>,
    cache: MarkdownCache,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let config = Config::from_env();

    // Create fetcher
    let fetch_config = FetchConfig {
        user_agent: config.user_agent.clone(),
        timeout_secs: config.request_timeout,
        max_content_length: config.max_content_length,
        max_redirects: config.max_redirects,
    };
    let fetcher = Fetcher::new(fetch_config)?;

    // Create cache
    let cache = MarkdownCache::new(config.cache_ttl);

    let state = AppState {
        fetcher: Arc::new(fetcher),
        cache,
    };

    // Build router
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/health", get(health_handler))
        .route("/{*url}", get(proxy_handler))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(&addr).await?;
    info!("mdwn.io listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Health check endpoint
async fn health_handler() -> &'static str {
    "OK"
}

/// Index handler - serve README
async fn index_handler() -> impl IntoResponse {
    let readme = include_str!("../readme.md");
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        readme,
    )
}

/// Main proxy handler
async fn proxy_handler(
    State(state): State<AppState>,
    Path(url_path): Path<String>,
) -> Response {
    match process_url(&state, &url_path).await {
        Ok((markdown, source)) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                "text/markdown; charset=utf-8".parse().expect("valid header value"),
            );
            headers.insert(
                "X-Mdwn-Source",
                source.as_header_value().parse().expect("valid header value"),
            );

            (StatusCode::OK, headers, markdown).into_response()
        }
        Err(e) => e.into_response(),
    }
}

/// Process a URL and return markdown content
async fn process_url(state: &AppState, url_path: &str) -> Result<(String, ContentSource)> {
    // Parse and validate URL
    let url = state.fetcher.parse_url(url_path)?;
    let url_str = url.as_str();

    // Check cache
    if let Some(cached) = state.cache.get(url_str).await {
        tracing::debug!("Cache hit for {}", url_str);
        return Ok((cached.markdown, cached.source));
    }

    // Fetch the URL
    let response = state.fetcher.fetch(&url).await?;

    // Process based on content type
    let (markdown, source) = match categorize_content_type(response.mime_type()) {
        ContentCategory::Markdown | ContentCategory::PlainText => {
            // Pass through directly
            (response.body_as_string(), ContentSource::Native)
        }

        ContentCategory::Html => {
            process_html(&state.fetcher, &response).await?
        }

        ContentCategory::Json => {
            process_json(&state.fetcher, &response).await?
        }

        ContentCategory::Unsupported(mime) => {
            return Err(MdwnError::UnsupportedType(mime));
        }
    };

    // Cache the result
    state.cache.set(url_str, markdown.clone(), source.clone()).await;

    Ok((markdown, source))
}

/// Process HTML response
async fn process_html(
    fetcher: &Fetcher,
    response: &fetch::FetchResponse,
) -> Result<(String, ContentSource)> {
    let html = response.body_as_string();

    // First, check for markdown link
    match parse_html_for_markdown_link(&html, &response.final_url)? {
        HtmlParseResult::MarkdownLink(md_url) => {
            // Fetch the linked markdown
            let md_response = fetcher.fetch(&md_url).await?;
            Ok((md_response.body_as_string(), ContentSource::Native))
        }

        HtmlParseResult::NeedsConversion => {
            // Check if HTML has meaningful content
            if !convert::is_meaningful_html(&html) {
                return Err(MdwnError::NoMarkdown(
                    "Page appears to require JavaScript to render content".to_string(),
                ));
            }

            // Convert HTML to markdown
            let markdown = convert::html_to_markdown(&html, &response.final_url)?;
            Ok((markdown, ContentSource::Converted))
        }
    }
}

/// Process JSON response
async fn process_json(
    fetcher: &Fetcher,
    response: &fetch::FetchResponse,
) -> Result<(String, ContentSource)> {
    let json = response.body_as_string();

    match parse_json_for_markdown(&json, &response.final_url)? {
        JsonParseResult::MarkdownUrl(md_url) => {
            // Fetch the linked markdown
            let md_response = fetcher.fetch(&md_url).await?;
            Ok((md_response.body_as_string(), ContentSource::Native))
        }

        JsonParseResult::MarkdownContent(content) => {
            // Return inline content directly
            Ok((content, ContentSource::Native))
        }

        JsonParseResult::NotFound => {
            Err(MdwnError::NoMarkdown(
                "JSON response has no 'mdwn' or 'markdown' field".to_string(),
            ))
        }
    }
}
